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

    const unsubscribe = status.subscribe(new_status => {
        if (new_status.connected && new_status.gcode_lines_done_total) {
            [current_gcode, lines_done, lines_total] = $status.gcode_lines_done_total;
            gcode_loaded = true;
        } else {
            current_gcode = NO_GCODE_MSG;
            gcode_loaded = false;
        }
    });

    onDestroy(unsubscribe);
</script>

<div class="container">
    <div class="current_file">{current_gcode}</div>

    {#if gcode_loaded}
        <progress class="progress_bar" value={lines_done / lines_total}></progress>

        {#if $status.state == "CONNECTED" || $status.state == "PAUSED" || $status.state == "DONE"}
            <button title="Start Printing" on:click={ () => {
                send_api_cmd("POST", "start_print")
                .catch((err) => {
                    alert(err);
                });
            }}>
                <img src={playIcon} width="50" height="50" alt="Play"/>
            </button>
        {/if}

        {#if $status.state == "STARTED" || $status.state == "PAUSED"}
            <button title="Stop Printing - this will cancel the current job!" on:click={()=> {
                send_api_cmd("POST", "stop_print")
                .catch((err) => {
                    alert(err);
                });
            }}>
                <img src={stopIcon} width="50" height="50" alt="Stop"/>
            </button>

            <button disabled={$status.state == "PAUSED"} title="Pause Printing - this will pause the current job. You may resume it afterwards" on:click={() => {
                send_api_cmd("POST", "pause_print")
                .catch((err) => {
                    alert(err);
                });
            }}>
                <img src={pauseIcon} width="50" height="50" alt="Stop"/>
            </button>
        {/if}
    {/if}
</div>

<style>
    .container {
        display: flex;
        flex-wrap: wrap;
        justify-content: center;
        width: fit-content;
        align-self: center;
    }

    .container > button {
        margin: 5px;
    }
    .container > div {
        margin: 5px;
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