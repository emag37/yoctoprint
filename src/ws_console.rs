use crate::internal_api::ConsoleMessage;

use crossbeam::channel::{Sender, Receiver};
use std::sync::Arc;

struct ConsoleFactory {
    n_connections: u32,
    to_from_printer: Arc<(Sender<ConsoleMessage>, Receiver<ConsoleMessage>)>,
}

struct ConsoleHandler {
    out: ws::Sender,
    to_from_printer: Arc<(Sender<ConsoleMessage>, Receiver<ConsoleMessage>)>,
}

pub struct WSConsole {
    worker_handle: std::thread::JoinHandle<()>,
    port: u16
}


impl ws::Factory for ConsoleFactory {
    fn connection_made(&mut self, out: ws::Sender) -> Self::Handler {
        if self.n_connections > 0 {
            warn!("Watch out! We have {} active connections to the same console, some weird behaviour may occur", self.n_connections);
        }
        self.n_connections += 1;
        return ConsoleHandler{out: out, to_from_printer: self.to_from_printer.clone()}
    }

    type Handler = ConsoleHandler;
}

impl ConsoleFactory {
    pub fn new(to_from_printer: (Sender<ConsoleMessage>, Receiver<ConsoleMessage>)) -> Self {
        ConsoleFactory{to_from_printer: Arc::new(to_from_printer), n_connections: 0}
    }
}


impl ws::Handler for ConsoleHandler {
    fn on_open(&mut self, shake: ws::Handshake) -> ws::Result<()> {
        if shake.peer_addr.is_some() {
            info!("Got WebSocket connection from {}", shake.peer_addr.unwrap());
        }
        self.out.timeout(10, WSConsole::CHECK_RX)?;
        Ok(())
    }
    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        match self.to_from_printer.0.try_send(ConsoleMessage{line: msg.to_string(), is_echo: false}) {
            Ok(()) => {}
            Err(e) => {
                if e.is_disconnected() {
                    if let Err(e) = self.out.close(ws::CloseCode::Normal) {
                        error!("Error, closing websocket: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    fn on_close(&mut self, code: ws::CloseCode, reason: &str) {
        info!("WebSocket closing for ({:?}) {}", code, reason);
        self.out.shutdown().unwrap();
    }
    fn on_timeout(&mut self, event: ws::util::Token) -> ws::Result<()> {
        match event {
            WSConsole::CHECK_RX => {
                loop {
                    match self.to_from_printer.1.try_recv() {
                        Ok(msg) => {
                            let as_json = rocket::serde::json::to_string(&msg).unwrap();
                            if let Err(e) = self.out.send(as_json) {
                                error!("Error sending message: {}", e);
                            }
                        }
                        Err(e) => {
                            if e.is_disconnected() {
                                if let Err(e) = self.out.close(ws::CloseCode::Normal) {
                                    error!("Error closing websocket: {}", e);
                                }
                            }
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
        self.out.timeout(10, WSConsole::CHECK_RX)?;
        Ok(())
    }
}

impl WSConsole {
    const CHECK_RX :ws::util::Token = ws::util::Token(1);

    pub fn new(to_from_printer: (Sender<ConsoleMessage>, Receiver<ConsoleMessage>)) -> WSConsole {
        let mut settings = ws::Settings::default();
            settings.tcp_nodelay = true;
            settings.panic_on_internal = false;
            let ws_server = ws::Builder::new().with_settings(settings).build(
                ConsoleFactory::new(to_from_printer)
            ).unwrap();
            let bound = ws_server.bind("0.0.0.0:0").unwrap();
            let port = bound.local_addr().unwrap().port();
            let handle = std::thread::spawn( move || {
                if let Err(e) = bound.run() {
                    error!("Error starting WebSocket: {}",e);
                }
            });
            WSConsole { worker_handle: handle, port: port }
    }
    
    pub fn is_open(&self) -> bool {
        return !self.worker_handle.is_finished();
    }

    pub fn port(&self) -> u16 {
        return self.port;
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use ws::{connect, CloseCode, Message};
    use rocket::serde::json;

    #[test]
    fn tx_rx_data() {
        let (rx_out, rx_in) = crossbeam::channel::unbounded::<ConsoleMessage>();
        let (tx_out, tx_in) = crossbeam::channel::unbounded::<ConsoleMessage>();
        let console_ws = WSConsole::new((tx_out, rx_in));
        
        std::thread::sleep(std::time::Duration::from_millis(10));

        let client = std::thread::spawn(move || {
            assert!(connect(format!("ws://127.0.0.1:{}", console_ws.port), |out| {
            assert!(out.send("M115\n").is_ok());
            
            move |msg: Message| {
                // Wait for one message (sent below) and close

                let recvd: ConsoleMessage = json::from_str(msg.to_string().as_str()).unwrap();
                assert_eq!(recvd, ConsoleMessage{line: "M110 N150\n".to_string(), is_echo: true});

                out.close(CloseCode::Normal)
            }
        }).is_ok())});
        
        let recv = tx_in.recv_timeout(std::time::Duration::from_secs(3));
        assert!(recv.is_ok());

        assert_eq!(recv.as_ref().unwrap().is_echo, false);
        assert_eq!(recv.as_ref().unwrap().line, "M115\n");

        assert!(rx_out.send(ConsoleMessage{line: "M110 N150\n".to_string(), is_echo:true}).is_ok());

        _ = client.join();
    }

    #[test]
    fn websocket_closed_when_crossbeam_closed() {
        let (rx_out, rx_in) = crossbeam::channel::unbounded::<ConsoleMessage>();
        let (tx_out, tx_in) = crossbeam::channel::unbounded::<ConsoleMessage>();
        let console_ws = WSConsole::new((tx_out, rx_in));
        
        std::thread::sleep(std::time::Duration::from_millis(10));

        let client = std::thread::spawn(move || {
            assert!(connect(format!("ws://127.0.0.1:{}", console_ws.port), |out| {
            
            move |_msg: Message| {
                // Wait for one message that will never come...

                out.close(CloseCode::Normal)
            }
        }).is_ok())});
        
        assert_eq!(console_ws.is_open(), true);
        drop(rx_out);

        _ = client.join();
        assert_eq!(console_ws.is_open(), false);
    }
}