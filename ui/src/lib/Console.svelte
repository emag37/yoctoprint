<script>
    import { api_url } from '../data';
    import { onMount } from 'svelte';

    const bufferDepth = 500;

    let socket = null;
    let inputText = "";
    let textLines = [];

    onMount(()=>{
        openConsole();
    });

    function sendInput() {
        socket.send(inputText);
        inputText = "";
    }

    function openConsole() {
        fetch(api_url() + "open_console", {method: "POST", keepalive: true})
        .then(resp => {
            return resp.text()
        }).then(port_str => {
            socket = new WebSocket(`ws://${window.location.hostname}:${port_str}`); 
            socket.onmessage = (msg_json) => {
                let msg = JSON.parse(msg_json.data);
                if (msg.is_echo) {
                    msg.line = `> ${msg.line}`
                }
                textLines.push(msg.line);
                while(textLines.length > bufferDepth) {
                    textLines.shift();
                }
                textLines = textLines;
            }
        })
        .catch(err => {
            console.log(err);
        });
    }
    
</script>

<div class="console_container">
    <textarea cols=50 rows=10 readonly="true" overflow="auto" class="output">{textLines.join("\n")}</textarea>

    <input class="input" type="text" bind:value="{inputText}"/>
    <button class="send" disabled={socket == null || socket.readyState != 1 || inputText.length === 0} on:click={sendInput}>Send</button>
</div>

<style>
    .console_container{
        max-width: 50%;
        display: grid;
        grid-template-columns: 10fr 1fr;
    }

    .console_container > textarea {
        background-color: lightgray;
    }

    .output {
        grid-column: 1 / span 2;
        grid-row: 1;
    }
    .input {
        grid-row: 2;
        grid-column: 1;
        width: 100%;
    }
    .send {
        grid-row: 2;
        grid-column: 2;
        max-width: min-content;
    }
</style>