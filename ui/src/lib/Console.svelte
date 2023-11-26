<script>
    import { console_url,  status } from '../data';
    import { tick } from 'svelte';
    import connectIcon from '../assets/connect.png';
    
    const bufferDepth = 500;

    const connectedColour = "#2ebc33"
    const disconnectedColour = "darkgrey"
    let socket = null;
    let inputText = "";
    let textLines = [];
    let textArea = null;
    let isConnected = false;

    function toggleConsole() {
        if (isConnected === true) {
            socket.close();
        } else {
            openConsole();
        }
    }

    function sendInput() {
        socket.send(inputText);
        inputText = "";
    }

    function openConsole() {
        isConnected = true;
        socket = new WebSocket(console_url()); 
        socket.onclose = (_ev) => {
            isConnected = false;
        }
        socket.onmessage = (msg_json) => {
            let msg = JSON.parse(msg_json.data);
            if (msg.is_echo) {
                msg.line = `> ${msg.line}`
            }
            textLines.push(msg.line);
            while(textLines.length > bufferDepth) {
                textLines.shift();
            }
            textLines = textLines; // Svelte pls.
            
            const wasBottom = textArea.scrollHeight === textArea.clientHeight || textArea.scrollTop + textArea.clientHeight >= textArea.scrollHeight - 100;
            if (wasBottom) {
                tick().then(() => {
                    textArea.scrollTop = textArea.scrollHeight - textArea.clientHeight;
                });
                
            }
        }
    }
    
</script>

<div class="window">
    <div class="header">
        <div style="grid-row:1; grid-column:1; font-size: large ">G-Code Console</div>
        <button style="grid-row:1; grid-column:2; background-color: {isConnected === true ? connectedColour : disconnectedColour}" title="Connect / Disconnect Console" on:click={toggleConsole}>
        <img width="25" height="25" alt="connect console" src={connectIcon}/>
    </button>
    </div>
    {#if isConnected === true}
    <div class="console_container">
        <textarea cols=50 rows=10 readonly="true" overflow="auto" class="output" bind:this={textArea}>{textLines.join("")}</textarea>

        <input class="input" type="text" bind:value="{inputText}"/>
        <button class="send" disabled={$status.state == "STARTED" || socket == null || socket.readyState != 1 || inputText.length === 0} on:click={sendInput}>Send</button>
    </div>
    {/if}
</div>
<style>
    .console_container{
        margin-top: 10px;
        display: grid;
        grid-template-columns: 10fr 1fr;
    }

    .console_container > textarea {
        background-color: lightgray;
    }
    .header {
        display:grid;
        justify-content: space-between;
        align-items: center;
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
    .window {
        text-align: left;
        border-width: 1px;
        border-style: solid;
        border-color: gray;
        padding:5px;
        border-radius: 5px;
        max-width: 50%;
    }
</style>