<script>
    import searchGif from '../assets/confused-john-travolta.gif'
    import {send_api_cmd} from '../data'

    function simPrinter() {
        send_api_cmd("POST", "connect", JSON.stringify({
            "port":"sim",
            "baud":0
        }));
    }
</script>

<style>
button{
    border-width: 3px;
    border-style: solid;
}
</style>

<h2>Looking for a printer...</h2>
<img src={searchGif} alt="Searching!"/>
<button on:click={simPrinter}>Simulate a Printer</button>