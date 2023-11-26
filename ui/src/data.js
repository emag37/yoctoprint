import { onMount } from 'svelte';
import { writable, readable, derived } from 'svelte/store';

let refreshStatus = null;

export function fetch_api(method, path, body = null) {
    return fetch(api_url() + path, {method: method,
    headers: {
        'Accept': 'application/json',
        'content-type' : 'application/json'
    },
    body : body, keepalive: true})
    .then(resp => resp.json())
}

export function send_api_cmd(method, path, body = null) {
    return fetch(api_url() + path, {method: method,
    headers: {
        'Accept': 'application/json',
        'content-type' : 'application/json'
    },
    body : body, keepalive: true})
    .then(refreshStatus)
}

export const server_addr = window.location.hostname;

export function console_url() {
    return 'ws://' + server_addr + ":5000/api/console";
}

export function api_url() {
    return 'http://' + server_addr + ":5000/api/";
}

const default_status = {
    "host_connected": false,
    "printer_connected": false, 
    "temperatures":[],
    "manual_control_enabled": false,
    "fan_speed":[0.]
}

const status = readable(default_status, (set) => {
    refreshStatus =  () => 
        fetch_api("GET", "status")
        .then(data => {
            data["host_connected"] = true;
            set(data)
        }).catch(err => {
            set(default_status);
            console.error(err);
        });

    let refreshInterval = () => refreshStatus().then(() => {setTimeout(() => {refreshInterval()}, 1000)});
    
    refreshInterval();
});


export {status}