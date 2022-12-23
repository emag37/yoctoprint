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
      <div class="files_temp">
        <div class="files">
          <Files />
        </div>
        <Temperature />  
      </div>
    </div>
  </Modal>
  
</main>

<style>
  .files_temp {
    display: flex;
    justify-content: space-between;
  }
  .files {
    flex-basis: 40%;
  }
</style>
