<script>
    
    import {Modal, bind} from 'svelte-simple-modal';
    import { writable, get } from 'svelte/store';
    import { send_api_cmd, status } from '../data';
    import playIcon from '../assets/play-circle.svg';
    import stopIcon from '../assets/stop-circle.svg';
    import pauseIcon from '../assets/pause-circle.svg';
    import { onDestroy } from 'svelte';
    import ConfirmationDialog from './ConfirmationDialog.svelte';
    import bananaManGif from '../assets/banana_man.webp'

    const NO_GCODE_MSG = "Nothing cooking 🍲";
    const confirmation_dialog = writable(null);

    let gcode_loaded = false;
    let current_gcode = NO_GCODE_MSG;
    let lines_done = 0;
    let lines_total = 1;
    let time_remaining;
    let time_elapsed = null;
    

    const PAUSEABLE_STATES = ["STARTED"];

    const STARTABLE_STATES = ["CONNECTED", "PAUSED", "DONE"];

    const STOPPABLE_STATES = ["STARTED", "PAUSED", "DONE"];

    function duration_to_string(duration) {
        let duration_secs = duration.secs + duration.nanos * 1e-9;
        return new Date(duration_secs * 1000).toISOString().slice(11,19);
    }

    const unsubscribe = status.subscribe(new_status => {
        if (new_status.host_connected && new_status.gcode_lines_done_total) {
            [current_gcode, lines_done, lines_total] = new_status.gcode_lines_done_total;
            gcode_loaded = true;
            time_remaining = duration_to_string(new_status.print_time_remaining);
        } else {
            current_gcode = NO_GCODE_MSG;
            gcode_loaded = false;
            time_remaining = "Unavailable";
        }

        if (new_status.print_time_elapsed != null) {
            time_elapsed = duration_to_string(new_status.print_time_elapsed);
        } else {
            time_elapsed = null;
        }
    });

    let bananaAnimRunning = false;
    function start_stop_banana_anim() {
        bananaAnimRunning = !bananaAnimRunning;
        setTimeout(start_stop_banana_anim, bananaAnimRunning ? 2000 : 10000);
    }

    start_stop_banana_anim();

    onDestroy(unsubscribe);

    function lowerCase(text) {
        text = text.toLowerCase();
        return text.charAt(0).toUpperCase() + text.slice(1);
    }

    function stop_print() {
        new Promise((resolve, reject) => {
            if ($status.state == "DONE") {
                resolve();
            }else {
                confirmation_dialog.set(bind(ConfirmationDialog, {message: "Watch out! This will ABORT the current print. Are you sure?",
                resolve_fn: resolve,
                reject_fn: reject}));
            }
        }).then(() => {
            confirmation_dialog.set(null);
            return send_api_cmd("POST", "stop_print")
        }, () => {
            confirmation_dialog.set(null);
        })
        .catch((err) => {
            alert(err);
        });

    }
</script>

<div class="container">
    <Modal show={$confirmation_dialog} closeOnOuterClick={false} closeButton={false} closeOnEsc={false} >
        <div class="current_file">{current_gcode}</div>

        {#if gcode_loaded}
            <progress class="progress_bar" value={lines_done / lines_total}></progress>

            <div class="time_remaining">{lowerCase($status.state)}, ETA: {time_remaining} {#if time_elapsed != null}, Elapsed: {time_elapsed}{/if}</div>
            {#if $status.state == "CONNECTED" || $status.state == "PAUSED" || $status.state == "DONE" || $status.state == "STARTED"}
                
                <button disabled={!STARTABLE_STATES.includes($status.state)} title="Start Printing" on:click={ () => {
                    send_api_cmd("POST", "start_print")
                    .catch((err) => {
                        alert(err);
                    });
                }}>
                    <img src={playIcon} width="50" height="50" alt="Play"/>
                </button>
                <button disabled={!STOPPABLE_STATES.includes($status.state)} title="Stop Printing - this will cancel the current job!" on:click={stop_print}>
                    <img src={stopIcon} width="50" height="50" alt="Stop"/>
                </button>

                <button disabled={!PAUSEABLE_STATES.includes($status.state)} title="Pause Printing - this will pause the current job. You may resume it afterwards" on:click={() => {
                    send_api_cmd("POST", "pause_print")
                    .catch((err) => {
                        alert(err);
                    });
                }}>  <img src={pauseIcon} width="50" height="50" alt="Stop"/>
                </button>
                {#if $status.state == "DONE"}
                    <img style="animation-play-state: {bananaAnimRunning ? "running" : "paused"}" class="banana_man" src={bananaManGif}/>
                {/if}
            {/if}
        {/if}
    </Modal>
</div>

<style>
    .container {
        display: flex;
        flex-wrap: wrap;
        justify-content: center;
        width: auto;
        align-self: center;
    }

    .container > button {
        margin: 5px;
    }
    .container > button:disabled {
        background-color: darkgrey;
    }
    .container > div {
        margin: 5px;
    }
    .time_remaining {
        flex-basis: 100%;
    }
    .current_file {
        font-size: xx-large;
        flex-basis: 100%;
    }

    .progress_bar {
        margin: 5px;
        flex-basis: 100%;
        width: min-content;
    }

    @keyframes anim-banana {
        0%   {right: 0px; }
        50%  {right: 200px; transform: translate(0, -100px) rotate(0.5turn) }
        100% {right: 400px; transform: rotate(1turn);}
    }

    .banana_man {
        position: relative;
        width: 75px;
        height: 75px;
        object-fit: fill;
        animation-name: anim-banana;
        animation-duration: 2s;
        animation-timing-function: linear;
        animation-iteration-count: infinite;
        animation-direction: alternate;
    }
</style>