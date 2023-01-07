<script>
    export let label;
    export let index;
    export let current_value;
    export let disabled;
    export let units;
    export let min;
    export let max;
    export let onChange;

    let desired_value = current_value;
</script>

<div class="num_adjust">
    <label for="set_num_{label}">{label} {index}</label>
    <input type="number" id="set_num_{label}" style="--digits: {max.toString().length}.5em" min={min} max={max} bind:value={desired_value}/>
    <span class="units">{units}</span>
    <button disabled={disabled} on:click={() => {
        onChange(label, index, desired_value);
    }}> Set</button>
</div>

<style>
.num_adjust {
    display: flex;
    flex-wrap: wrap;
    align-self: center;
    border-radius: 10px;
    padding: 10px;
    max-width: fit-content;
    align-items: center;
}

.num_adjust > label {
    flex-basis: 100%;
    font-weight: bolder;
}
.num_adjust > button {
    margin-left: auto;
}
.num_adjust > input {
    font-size: large;
    max-width: var(--digits);
    text-align: center;
}

.units {
    font-weight: bolder;
    padding-left: 3px;
    padding-top: 3px;
    font-weight: bolder;
    font-size: large;
}
</style>