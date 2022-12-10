<script>
  import Control from './lib/Control.svelte'
  import Temperature from './lib/Temperature.svelte';
  import Files from './lib/Files.svelte';
  import PrintJob from './lib/PrintJob.svelte';
</script>

<main>
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
