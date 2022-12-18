<script>
  import { writable, get } from 'svelte/store';
  import { status } from "./data";
  import Control from './lib/Control.svelte'
  import Temperature from './lib/Temperature.svelte';
  import Files from './lib/Files.svelte';
  import PrintJob from './lib/PrintJob.svelte';

  import Modal from 'svelte-simple-modal';
  import YoctoprintConnectionModal from './lib/YoctoprintConnectionModal.svelte';
  import PrinterConnectionModal from './lib/PrinterConnectionModal.svelte';
  const modal = writable(null);

  $: {
    if ($status.host_connected === false) {
      if (get(modal) != YoctoprintConnectionModal) {
        modal.set(YoctoprintConnectionModal);
      }
    } else if ($status.printer_connected === false) {
      if (get(modal) != PrinterConnectionModal) {
        modal.set(PrinterConnectionModal);
      }
    } else {
      modal.set(null);
    }
  }
</script>

<main>
  <Modal show={$modal} closeOnOuterClick={false} closeButton={false} closeOnEsc={false} >
    <div>
      <div class="card">
        <PrintJob />
      </div>
      <div class="card">
        <Control />
      </div>
      <div class="card">
        <Temperature />
      </div>
      <div class="card">
        <Files />
      </div>
    </div>
  </Modal>
  
</main>

<style>
  .logo {
    height: 6em;
    padding: 1.5em;
    will-change: filter;
  }
  .logo:hover {
    filter: drop-shadow(0 0 2em #646cffaa);
  }
  .logo.svelte:hover {
    filter: drop-shadow(0 0 2em #ff3e00aa);
  }
  .read-the-docs {
    color: #888;
  }
</style>
