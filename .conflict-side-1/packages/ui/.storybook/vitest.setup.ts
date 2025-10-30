import * as projectAnnotations from './preview';
import { setProjectAnnotations } from '@storybook/sveltekit';
import { beforeAll } from 'vitest';

const project = setProjectAnnotations(projectAnnotations);

beforeAll(project.beforeAll);
