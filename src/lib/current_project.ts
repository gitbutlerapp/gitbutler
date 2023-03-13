import { writable } from 'svelte/store';
import type { Project } from '$lib/projects';

export const currentProject = writable<Project | undefined>(undefined);
