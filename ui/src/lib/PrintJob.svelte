<script>
    import { send_api_cmd, status } from '../data';
    import playIcon from '../assets/play-circle.svg';
    import stopIcon from '../assets/stop-circle.svg';
    import pauseIcon from '../assets/pause-circle.svg';
    import { onDestroy } from 'svelte';

    const NO_GCODE_MSG = "Nothing cooking 🍲";

    let gcode_loaded = false;
    let current_gcode = NO_GCODE_MSG;
    let lines_done = 0;
    let lines_total = 1;
    let time_remaining;

    const unsubscribe = status.subscribe(new_status => {
        if (new_status.host_connected && new_status.gcode_lines_done_total) {
            [current_gcode, lines_done, lines_total] = new_status.gcode_lines_done_total;
            gcode_loaded = true;
            let time_remaining_secs = new_status.print_time_remaining.secs + new_status.print_time_remaining.nanos * 1e-9;
            time_remaining = new Date(time_remaining_secs * 1000).toISOString().slice(11,19);
        } else {
            current_gcode = NO_GCODE_MSG;
            gcode_loaded = false;
            time_remaining = "Unavailable";
        }
    });

    onDestroy(unsubscribe);

    function lowerCase(text) {
        text = text.toLowerCase();
        return text.charAt(0).toUpperCase() + text.slice(1);
    }

    const PAUSEABLE_STATES = ["STARTED"];

    const STARTABLE_STATES = ["CONNECTED", "PAUSED", "DONE"];

    const STOPPABLE_STATES = ["STARTED", "PAUSED", "DONE"];
</script>

<div class="container">
    <div class="current_file">{current_gcode}</div>

    {#if gcode_loaded}
        <progress class="progress_bar" value={lines_done / lines_total}></progress>

        <div class="time_remaining">{lowerCase($status.state)}, ETA: {time_remaining}</div>
        {#if $status.state == "CONNECTED" || $status.state == "PAUSED" || $status.state == "DONE" || $status.state == "STARTED"}
            
            <button disabled={!STARTABLE_STATES.includes($status.state)} title="Start Printing" on:click={ () => {
                send_api_cmd("POST", "start_print")
                .catch((err) => {
                    alert(err);
                });
            }}>
                <img src={playIcon} width="50" height="50" alt="Play"/>
            </button>
            <button disabled={!STOPPABLE_STATES.includes($status.state)} title="Stop Printing - this will cancel the current job!" on:click={()=> {
                send_api_cmd("POST", "stop_print")
                .catch((err) => {
                    alert(err);
                });
            }}>
                <img src={stopIcon} width="50" height="50" alt="Stop"/>
            </button>

            <button disabled={!PAUSEABLE_STATES.includes($status.state)} title="Pause Printing - this will pause the current job. You may resume it afterwards" on:click={() => {
                send_api_cmd("POST", "pause_print")
                .catch((err) => {
                    alert(err);
                });
            }}>  <img src={pauseIcon} width="50" height="50" alt="Stop"/>
            </button>
        {/if}
    {/if}
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
</style>