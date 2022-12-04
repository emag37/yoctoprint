<script>
    import { status } from "../data";
    import { onDestroy } from 'svelte';

let temperatures = []

const unsubscribe = status.subscribe(new_status => {
    if (new_status.connected) {
        temperatures = new_status.temperatures;
        for (let temp of temperatures) {
            temp.target_low =  temp.target - 5;
            temp.target_high = temp.target + 5;
            temp.current = temp.current.toFixed(2);
            temp.target = temp.target.toFixed(2);
        }
    }
});

onDestroy(unsubscribe);

</script>

{#each temperatures as temperature}
<div class="readout">
    <div class="probe">{temperature.measured_from +  " " + temperature.index}</div>
    <div class="sensor">
            <meter id="current_temp" class="meter" value={temperature.current} min="0" max="300" optimum={temperature.target} low={temperature.target_low} high={temperature.target_high}/>
            <label class="digital" for="current_temp">Current: {temperature.current}ºC</label>

            <meter id="target_temp" class="meter" value={temperature.target} min="0" max="300" low="75" high="220"/>
            <label class="digital" for="current_temp">Target: {temperature.target}ºC</label>
            
            <meter id="power" class="meter" value={temperature.power} min="0" max="128"/>
            <label class="digital" for="power">Power: {temperature.power}</label>
    </div>
</div>
{/each}

<style>
.readout {
    padding-bottom: 5px;
}
.sensor {
    display: grid;
    width: auto;
    clear: both;
    padding: 5px;
    border-style: solid;
    border-width: 1px;
    border-radius: 5px;
    border-color: gray;
}

.probe {
    float:left;
    border-width: 1px;
    border-style: solid;
    border-color: gray;
    padding:3px;
    border-radius: 5px;
}
.digital {
    grid-column: 2;
    padding-left: 0.5em;
    text-align: left;
}

.meter { 
  grid-column: 1;
  height: 2em;
  width: 300px;
  position: relative;
  background: none;
  padding-top: 1px;
  padding-bottom: 1px;
}

.yellow {
  background-color: #efe300;
}

.orange{
  background-color: #ef8300;
}

</style>