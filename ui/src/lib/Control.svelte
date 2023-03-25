<script>
import homeIcon from '../assets/home.svg';
import arrowUp from '../assets/arrow-up-bold-circle.svg';
import arrowDown from '../assets/arrow-down-bold-circle.svg';
import arrowLeft from '../assets/arrow-left-bold-circle.svg';
import arrowRight from '../assets/arrow-right-bold-circle.svg';
import extrudeIcon from '../assets/extrude.svg';
import retractIcon from '../assets/retract.svg';
import NumberControl from './NumberControl.svelte';
import { send_api_cmd, status } from '../data';

const step_map = [0.1, 1, 2, 3, 4, 5, 10, 20];

let current_step = 0.1;
let step_input = 2;
let current_feed = 10;

let request_active = false;
let can_home;
let can_control;

$: can_home = $status.state !== "STARTED" && !request_active;
$: can_control = $status.manual_control_enabled === true && can_home;

function validateFeed() {
    if (current_feed < 0.1) {
        current_feed = 0.1;
    } else if (current_feed > 100) {
        current_feed = 100;
    }
}

function setFanSpeed(id, index, new_speed) {
    send_api_cmd("POST", "set_fan_speed", JSON.stringify({
        "index" : index,
        "speed" : new_speed / 100.
    })).catch((err) => {
        alert(err);
    });
}
function adjustStep() {
    current_step = step_map[step_input];
}

function move(axis, direction) {
    request_active = true;
    let obj = {};
    let dist = 0;

    axis = axis.toLowerCase();
    if (axis == "e") {
        dist = current_feed;
    } else {
        dist = current_step;
    }

    if (direction == "-") {
        dist *= -1;
    }

    obj[axis] = dist;
    send_api_cmd("POST","move", JSON.stringify(obj))
    .catch((err) => {
        alert(err);
    })
    .finally(() => {
        request_active = false;
    });
}

function home(axes) {
    request_active = true;
    send_api_cmd("POST", "home", JSON.stringify({
        "axes": axes
    })).catch((err) => {
        alert(err);
    })
    .finally(() => {
        request_active = false;
    });
}

function setTemperature(id, index, new_temp) {
    send_api_cmd("POST", "set_temperature", JSON.stringify({
        "to_set": id,
        "index" : index - 1,
        "target" : new_temp
    })).catch((err) => {
        alert(err);
    });
}
</script>

<div class="controls_container">
    <div class="xyz_container">
        <div class="title" style="grid-row: 1; grid-column: 1/ span 3">X/Y</div>
        <button disabled={!can_control} class="control_button" style="grid-column: 2; grid-row: 2" on:click={() => move("Y", "+")} > 
            <img class="control_button_img" src={arrowUp}  alt="Y+"/>
        </button>
        <button disabled={!can_control} class="control_button" style="grid-column: 1; grid-row: 3" on:click={() => move("X", "-")} > 
            <img class="control_button_img" src={arrowLeft} alt="X-"/>
        </button>
        <button disabled={!can_home} class="control_button" style="grid-column: 2; grid-row: 3" on:click={() => home(["X", "Y"])} > 
            <img class="control_button_img" src={homeIcon} alt="Home X/Y"/>
        </button>
        <button disabled={!can_control} class="control_button" style="grid-column: 3; grid-row: 3" on:click={() => move("X", "+")}> 
            <img class="control_button_img" src={arrowRight} alt="X+"/>
        </button>
        <button disabled={!can_control} class="control_button" style="grid-column: 2; grid-row: 4" on:click={() => move("Y", "-")}> 
            <img class="control_button_img" src={arrowDown}  alt="Y-"/>
        </button>

        <div class="spacer" style="grid-column: 4"></div>

        <div class="title" style="grid-row: 1; grid-column: 5">Z</div>
        <button disabled={!can_control} class="control_button" style="grid-column: 5; grid-row: 2" on:click={() => move("Z", "+")}> 
            <img class="control_button_img" src={arrowUp} alt="Z+"/>
        </button>
        <button disabled={!can_home} class="control_button" style="grid-column: 5; grid-row: 3" on:click={() => home(["Z"])}> 
            <img class="control_button_img" src={homeIcon}  alt="Home Z"/>
        </button>
        <button disabled={!can_control} class="control_button" style="grid-column: 5; grid-row: 4" on:click={() => move("Z", "-")}> 
            <img class="control_button_img" src={arrowDown}  alt="Z-"/>
        </button>

        <div class="step_adjust">
            <input type="range" min="0" max="{step_map.length - 1}" bind:value={step_input} on:input={adjustStep}/>
            <div>{current_step}mm</div>
        </div>

        <div class="spacer" style="grid-column: 6"></div>

        <div class="title" style="grid-row: 1; grid-column: 7">Tool (E)</div>
        
        <button disabled={!can_control} class="control_button" style="grid-column: 7; grid-row: 2 on:click={() => move("E", "-")}">
            <img class="control_button_img" src={retractIcon}  alt="Retract"/>
        </button>

        <div class="feed_adjust">
            <input type="number" id="feed" min="0.1" max="100" step="1" on:change={validateFeed} bind:value={current_feed}>
            <label for="feed">mm</label>
        </div>
        
        <button disabled={!can_control} class="control_button" style="grid-column: 7; grid-row: 4" on:click={() => move("E", "+")}>
            <img class="control_button_img" src={extrudeIcon}  alt="Extrude"/>
        </button>

        <div class="spacer" style="grid-column: 8"></div>
        <div class="title" style="grid-row: 1; grid-column: 9;">Heater/Fan</div>
        <div class="temp_container">
            {#each $status.temperatures as temperature}
                <NumberControl label={temperature.measured_from} index={temperature.index + 1} min=0 max=300 disabled={$status.state === "STARTED"} current_value={temperature.current} units="°C" onChange={setTemperature}/>
            {/each}
        </div>

        <div class="fan_container">
            {#each $status.fan_speed as speed, idx}
                <NumberControl label={"Fan"} index={idx} min=0 max=100 disabled={$status.state === "STARTED"} current_value={speed * 100.} units="%" onChange={setFanSpeed}/>
            {/each}
        </div>
    </div>

</div>

<style>

.title {
    font-size: xx-large;
    box-sizing: border-box;
    padding: 5px;
    border-bottom: solid;
    border-width: 2px;
}
.controls_container {
    display: grid;
    
    grid-row: 1;
    grid-column: 1;
    border:solid;
    border-radius: 10px;
    border-width: 1px;
    padding: 10px;
    max-width: min-content;
    margin-left: auto;
    margin-right: auto;
}

.step_adjust {
    grid-row: 5;
    grid-column: 1 / span 5;
    display: flex;
    justify-content: space-between;
    font-size: x-large;
}

.step_adjust > input {
    flex-basis: 75%;
}

.feed_adjust {
    display: flex;
    grid-column: 7;
    grid-row: 3;
    max-height: 1.5em;
    justify-content: space-between;
    align-self: center;
    border-radius: 10px;
    padding: 10px;
}

.feed_adjust > input{
    font-size: x-large;
    text-align: center;
    padding-right: 5px;
    flex-basis: 20%;
    max-width: 3em;
}
.feed_adjust > label {
    vertical-align: text-bottom;
    font-size: large;
}

.temp_container {
    display: block;
    grid-row: 2/ span 3;
    grid-column: 9;
    overflow-y: auto;
}

.xyz_container {
    grid-template-columns: repeat(7, auto) 10px 200px;
    grid-row: 1;
    grid-column: 1;
    grid-gap: 10px;
    display: inline-grid;
}

.control_button_img {
    width: 50px;
    height: 50px;
}

.spacer {
    width:10px;
    grid-row: 1 / span 5;
}

.fan_container {
    display: block;
    grid-row: 4/span 2;
    grid-column: 9;
    overflow-y: auto;
}

</style>
