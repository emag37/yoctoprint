<script>
import homeIcon from '../assets/home.svg';
import arrowUp from '../assets/arrow-up-bold-circle.svg';
import arrowDown from '../assets/arrow-down-bold-circle.svg';
import arrowLeft from '../assets/arrow-left-bold-circle.svg';
import arrowRight from '../assets/arrow-right-bold-circle.svg';
import { send_api_cmd, status } from '../data';
import { onMount } from 'svelte';

let current_step = 0.1;
let step_input = 2;
let current_feed = 10;

let request_active = false;
let controls_disabled
$: controls_disabled = $status.manual_control_enabled === false;
let desired_temperatures = {}

onMount(initTemperatures);

function validateFeed() {
    if (current_feed < 0.1) {
        current_feed = 0.1;
    } else if (current_feed > 100) {
        current_feed = 100;
    }
}

function initTemperatures() {
    if ($status.temperatures === undefined) {
        setTimeout(initTemperatures, 0.2);
    } else {
        desired_temperatures = Object.fromEntries($status.temperatures.map((temp) => {
            return [temp.measured_from, temp.current];
        }));
    }
}

function adjustStep() {
    switch(step_input) {
        case 1:
            current_step = 0.1;
            break;
        case 2:
            current_step = 1;
            break;
        case 3:
            current_step = 10;
            break;
        case 4:
            current_step = 20;
            break;
    }
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

function set_temperature(id, index, new_temp) {
    send_api_cmd("POST", "set_temperature", JSON.stringify({
        "to_set": id,
        "index" : index,
        "target" : new_temp
    })).catch((err) => {
        alert(err);
    });
}
</script>

<div class="controls_container">
    <div class="xyz_container">
        <div class="title" style="grid-row: 1; grid-column: 1/ span 3">X/Y</div>
        <button disabled={controls_disabled} class="control_button" style="grid-column: 2; grid-row: 2" on:click={() => move("Y", "+")} > 
            <img class="control_button_img" src={arrowUp}  alt="Y+"/>
        </button>
        <button disabled={controls_disabled} class="control_button" style="grid-column: 1; grid-row: 3" on:click={() => move("X", "-")} > 
            <img class="control_button_img" src={arrowLeft} alt="X-"/>
        </button>
        <button disabled={request_active} class="control_button" style="grid-column: 2; grid-row: 3" on:click={() => home(["X", "Y"])} > 
            <img class="control_button_img" src={homeIcon} alt="Home X/Y"/>
        </button>
        <button disabled={controls_disabled} class="control_button" style="grid-column: 3; grid-row: 3" on:click={() => move("X", "+")}> 
            <img class="control_button_img" src={arrowRight} alt="X+"/>
        </button>
        <button disabled={controls_disabled} class="control_button" style="grid-column: 2; grid-row: 4" on:click={() => move("Y", "-")}> 
            <img class="control_button_img" src={arrowDown}  alt="Y-"/>
        </button>

        <div style="width: 10px; grid-column: 4"></div>

        <div class="title" style="grid-row: 1; grid-column: 5">Z</div>
        <button disabled={controls_disabled} class="control_button" style="grid-column: 5; grid-row: 2" on:click={() => move("Z", "+")}> 
            <img class="control_button_img" src={arrowUp} alt="Z+"/>
        </button>
        <button disabled={request_active} class="control_button" style="grid-column: 5; grid-row: 3" on:click={() => home(["Z"])}> 
            <img class="control_button_img" src={homeIcon}  alt="Home Z"/>
        </button>
        <button disabled={controls_disabled} class="control_button" style="grid-column: 5; grid-row: 4" on:click={() => move("Z", "-")}> 
            <img class="control_button_img" src={arrowDown}  alt="Z-"/>
        </button>

        <div class="step_adjust">
            <input type="range" min="1" max="4" bind:value={step_input} on:input={adjustStep}/>
            <div>{current_step}mm</div>
        </div>

        <div style="width: 10px; grid-column: 6"></div>

        <div class="title" style="grid-row: 1; grid-column: 7">Tool (E)</div>
        
        <button disabled={controls_disabled} class="control_button" style="grid-column: 7; grid-row: 2" on:click={() => move("E", "+")}>Extrude</button>
        <div class="feed_adjust">
            <input type="number" id="feed" min="0.1" max="100" step="0.1" on:change={validateFeed} bind:value={current_feed}>
            <label for="feed">mm</label>
        </div>
        
        <button disabled={controls_disabled} class="control_button" style="grid-column: 7; grid-row: 4 on:click={() => move("E", "-")}">Retract</button>

        <div style="width: 10px; grid-column: 8"></div>
        <div class="title" style="grid-row: 1; grid-column: 9;">Heater</div>
        {#each $status.temperatures as temperature, idx}
        <div class="temp_adjust" style="grid-row: {2 + idx}; grid-column:9;">
            <label for="set_temp_{temperature.measured_from}">{temperature.measured_from}</label>
            <input type="number" id="set_temp_{temperature.measured_from}" min="0" max="300" bind:value={desired_temperatures[temperature.measured_from]}/>
            <div>°C</div>
            <button disabled={$status.state === "STARTED"} on:click={() => {
                set_temperature(temperature.measured_from, temperature.index, desired_temperatures[temperature.measured_from]);
            }}>Set</button>
        </div>
        {/each}
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
    background-color: gray;
    border-radius: 10px;
    padding: 10px;
}

.feed_adjust > input{
    font-size: x-large;
    text-align: center;
    padding-right: 5px;
    flex-basis: 20%;
    width:75px;
}
.feed_adjust > label {
    vertical-align: text-bottom;
    font-size: large;
}

.temp_adjust {
    display: flex;
    flex-wrap: wrap;
    grid-column: 9;
    align-self: center;
    background-color: gray;
    border-radius: 10px;
    padding: 10px;
}

.temp_adjust > label {
    flex-basis: 100%;
    font-weight: bolder;
}
.temp_adjust > button {
    margin-left: auto;
}
.temp_adjust > input {
    font-size: large;
    text-align: center;
}

.xyz_container {
    grid-template-columns: repeat(6, auto) auto 10px 200px;
    grid-row: 1;
    grid-column: 1;
    grid-gap: 10px;
    display: inline-grid;
}

.control_button:disabled {
    background-color: darkgrey;
}
.control_button_img {
    width: 50px;
    height: 50px;
}

</style>
