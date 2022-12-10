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

const status = readable({
    "connected": false, 
    "temperatures":[],
    "manual_control_enabled": false,
    }, (set) => {
    let incrementCounter = setInterval( () => {
        fetch_api("GET", "status")
        .then(data => {
            set(data)
        }).catch(err => {
            console.error(err);
        });
    }, 1000);
    return () => {
      clearInterval(incrementCounter);
    };
  });


export {status}