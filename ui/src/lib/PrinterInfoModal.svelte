<script>
    import { onMount } from 'svelte';
    import {fetch_api} from '../data';
    
    let values = [];

    onMount(()=>{
        fetch_api("GET", "printer_info")
        .then(data => {
            values = Object.entries(data["values"]);
            values.sort((a,b) => {
                return a[0].localeCompare(b[0]);
            });
        }).catch(err => {
            console.error(err);
        });
    });
    
</script>

<div style="height: 400px">
<table class="nicetable">
    <tr>
        <th>Property</th>
        <th>Value</th>
    </tr>
    {#each values as value}
        <tr>
            <td>{value[0]}</td>
            <td>{value[1]}</td>
        </tr>
    {/each}
</table>
</div>

<style>
table.nicetable {
  background-color: #A1E8FF;
  text-align: center;
  border-collapse: collapse;
}
table.nicetable td, table.nicetable th {
  border: 0px solid #555555;
  padding: 5px 10px;
}
table.nicetable tbody td {
  font-size: 12px;
  font-weight: bold;
  color: #000000;
}
table.nicetable tr:nth-child(even) {
  background: #398AA4;
}
table.nicetable tr:nth-child(odd) {
    background-color: #A1E8FF;
}
table.nicetable thead {
  background: #398AA4;
  border-bottom: 10px solid #398AA4;
}
table.nicetable thead th {
  font-size: 15px;
  font-weight: bold;
  color: #FFFFFF;
  text-align: left;
  border-left: 2px solid #398AA4;
}
table.nicetable thead th:first-child {
  border-left: none;
}

table.nicetable tfoot td {
  font-size: 13px;
}
table.nicetable tfoot .links {
  text-align: right;
}
table.nicetable tfoot .links a{
  display: inline-block;
  background: #FFFFFF;
  color: #398AA4;
  padding: 2px 8px;
  border-radius: 5px;
}
</style>