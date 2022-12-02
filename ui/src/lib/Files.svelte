<script>
    import {onMount} from 'svelte';
    import {fetch_api} from '../data';

    let files = []

    onMount(()=>{
        fetch_api("GET", "list_gcode")
        .then(data => {
            files = data.files;
        })
        .catch(err => {
            console.error(err);
        });
    })
</script>