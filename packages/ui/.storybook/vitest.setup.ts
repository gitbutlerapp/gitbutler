import { beforeAll } from 'vitest'
import { setProjectAnnotations } from '@storybook/sveltekit'
import * as projectAnnotations from './preview'

const project = setProjectAnnotations(projectAnnotations)

beforeAll(project.beforeAll)