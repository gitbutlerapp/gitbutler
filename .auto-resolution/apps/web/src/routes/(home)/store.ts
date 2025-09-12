import * as jsonLinks from '$lib/data/links.json';
import { writable } from 'svelte/store';

export const targetDownload = writable(jsonLinks.downloads.appleSilicon);
export const latestClientVersion = writable('...');
