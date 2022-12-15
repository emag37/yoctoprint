<script>
    import {onMount} from 'svelte';
    import {fetch_api, api_url, status} from '../data';
    import shredderIcon from '../assets/shredder.svg';
    import uploadIcon from '../assets/upload.svg';

    let files = []
    let uploadProgress = 0.

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
                file.name = file.name.replace(".gcode", "");
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
        filename += ".gcode";
        
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
        filename = `${filename}.gcode`
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

    function uploadFile() {
        const input = document.createElement('input');
        input.type = 'file';
        input.accept = '.gcode'

        input.onchange = e => { 
            let file = e.target.files[0]; 
            var reader = new FileReader();
            uploadProgress = 0.1;
            reader.readAsBinaryString(file,'UTF-8');

            // here we tell the reader what to do when it's done reading...
            reader.onload = readerEvent => {
                let content = readerEvent.target.result; // this is the content!
                var request = new XMLHttpRequest();
                request.onerror = (err) => {
                    alert(`Error uploading file: ${err}`);
                }
                request.upload.onprogress = (progress) => {
                    uploadProgress = (progress.loaded / progress.total) * 100.;
                }
                request.onload = (done) => {
                    uploadProgress = 0.;
                    refreshFileList();
                }
                
                request.open("PUT", `${api_url()}upload_gcode?filename=${file.name}`);
                request.setRequestHeader("content-type", "application/octet-stream")
                request.send(content);
            }

            reader.onerror = () => {uploadProgress = 0.;}
            reader.onabort = () => {uploadProgress = 0.;}
        }
        input.click();
    }
</script>

<div class="filecontainer">
<div class="title">
    <button disabled={uploadProgress > 0.} style="--upload_progress: {uploadProgress}%"  title="Upload a new GCode file" on:click={uploadFile}>
        <img src={uploadIcon} width="30" height="30" alt="Upload"/>
    </button>
</div>
{#if files.length == 0}
<div class="fileobj">
    <div class="fileinfo">
       <div> I don't have any G-Code files to print 😔</div>
    </div>
</div>
{:else}
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
                <button class="button" title="Select this file for printing" disabled={$status.state != "CONNECTED" || ($status.gcode_lines_done_total && $status.gcode_lines_done_total[0] == `${file.name}.gcode`)} on:click={() => {selectFile(file.name);}}>Select</button>
            </div>
        </div>
    {/each}
{/if}
</div>

<style>
    .filecontainer {
        max-width: 300px;
    }
    .title {
        display: flex;
        font-weight: bold;
        max-height: 3em;
        justify-content: space-between;
    }

    .title > button {
        background-image: conic-gradient(rgb(36, 255, 255) var(--upload_progress), 0, rgb(36, 171, 167) calc(100% - var(--upload_progress)));
    }

    .fileobj {
        border-style: solid;
        border-width: 1px;
        border-radius: 10px;
        display: block;
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