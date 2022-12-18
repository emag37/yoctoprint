import { onMount } from 'svelte';
import { writable, readable, derived } from 'svelte/store';

export function fetch_api(method, path, body = null) {
    return fetch(api_url() + path, {method: method,
    headers: {
        'Accept': 'application/json',
        'content-type' : 'application/json'
    },
    body : body})
    .then(resp => resp.json())
}

export function send_api_cmd(method, path, body = null) {
    return fetch(api_url() + path, {method: method,
    headers: {
        'Accept': 'application/json',
        'content-type' : 'application/json'
    },
    body : body})
}

export function api_url() {
    return 'http://' + window.location.hostname + ":5000/api/";
}

const default_status = {
    "host_connected": false,
    "printer_connected": false, 
    "temperatures":[],
    "manual_control_enabled": false,
}

const status = readable(default_status, (set) => {
    let refresh_status = () => {
        fetch_api("GET", "status")
        .then(data => {
            data["host_connected"] = true;
            set(data)
        }).catch(err => {
            set(default_status);
            console.error(err);
        }).then(() => {
            setTimeout(refresh_status, 1000)
        });
    }
    refresh_status();
  });


export {status}