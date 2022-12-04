<script>
    import {onMount} from 'svelte';
    import {fetch_api, api_url, status} from '../data';
    import shredderIcon from '../assets/shredder.svg';

    let files = []

    function formatBytes(bytes) {
        if (!+bytes) return '0'

        const k = 1024
        const sizes = ['', 'K', 'M', 'G']

        const i = Math.floor(Math.log(bytes) / Math.log(k))

        return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))}${sizes[i]}`
    }

    function refreshFileList() {
        fetch_api("GET", "list_gcode")
        .then(data => {
            files = data.files;

            for (let file of files) {
                file.size = formatBytes(file.size);
            }
        })
        .catch(err => {
            console.error(err);
        });
    }

    onMount(()=>{
        refreshFileList();
    })

    function deleteFile(filename) {
        fetch(api_url() + `delete_gcode?filename=${filename}`, {method: "DELETE"})
        .then(resp => {
            if(!resp.ok)
            {
                return resp.text().then((text) => {
                    throw new Error(`Error deleting file ${text}`)    
                });
            }
        })
        .catch(err => {
            alert(err);
        }).finally(() => {
            refreshFileList();
        });
    }

    function selectFile(filename) {
        fetch(api_url() + `set_gcode?filename=${filename}`, {method: "POST"})
        .then(resp => {
            if(!resp.ok)
            {
                return resp.text().then((text) => {
                    throw new Error(`Error selecting file ${text}`)    
                });
            }
        })
        .catch(err => {
            alert(err);
        }).finally(() => {
            refreshFileList();
        });
    }
</script>

{#each files as file}
    <div class="fileobj">
        <div class="fileinfo">
            <div class="filename">{file.name}</div>
            <div class="size">{file.size}</div>
        </div>
        
        <div class="filecommands">
            <button class="button" disabled={$status.state != "CONNECTED"} title="Erase this G-Code file" on:click={() => {deleteFile(file.name);}}>
                <img src={shredderIcon} width="20" height="20" alt="Delete"/>
            </button>
            <button class="button" title="Select this file for printing" disabled={$status.state != "CONNECTED" || $status.gcode_lines_done_total[0] == file.name} on:click={() => {selectFile(file.name);}}>Select</button>
        </div>
    </div>
{/each}

<style>
    .fileobj {
        border-style: solid;
        border-width: 1px;
        border-radius: 10px;
        display: block;
        max-width: 300px;
        padding:3px;
    }
    .fileinfo {
        padding:3px;
        display: flex;
        justify-content: space-between;
    }
    .filename {
        text-align: start;
        overflow-x: hidden;
        font-weight: bold;
    }
    .size {
        text-align: end;
    }
    .filecommands {
        padding:3px;
        display: flex;
        justify-content: flex-end;
        gap:3px;
    }
    .button {
        padding:0.3em;
    }
</style>