<script>
import aboutTableIcon from '../assets/about_table.svg';
import PrinterInfoModal from './PrinterInfoModal.svelte';
import Modal from 'svelte-simple-modal';
import { writable, get } from 'svelte/store';

const modal = writable(null);

</script>

<nav class="navbar">
    <button class="item" title="About This Printer" on:click={() => {
        modal.set(PrinterInfoModal);
    }}>
        <img class="nav_button" src={aboutTableIcon}  alt="About"/>
    </button>
</nav>

<Modal show={$modal} closeOnOuterClick={true} closeButton={true} closeOnEsc={true} >

</Modal>
<style>
    .navbar {
        display: flex;
    }
    .item {
        text-align: start;
        padding: 0px;
    }
    .nav_button {
        width: 30px;
        height: 30px;
    }
</style>