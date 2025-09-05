import MockBackend from '../mock/backend';
import {
	isGetBaseBranchArgs,
	isSetBaseBranchArgs,
	mockBaseBranchData,
	type BaseBranchData
} from '../mock/baseBranch';
import { createMockProject, isAddProjectArgs, isGetProjectArgs } from '../mock/projects';
import type { Project } from '$lib/project/project';
import type { InvokeArgs } from '@tauri-apps/api/core';

export default class FreshStart extends MockBackend {
	projectId = '2';
	private projects: Project[];
	private baseBranchData: Map<string, BaseBranchData>;

	constructor() {
		super();
		this.projects = [];
		this.stackDetails = new Map();
		this.stacks = [];
		this.baseBranchData = new Map();
	}

	listProjects(): Project[] {
		return this.projects;
	}

	addProject(args: InvokeArgs | undefined): Project {
		if (!args || !isAddProjectArgs(args)) {
			throw new Error('Invalid arguments for adding a project');
		}
		const projectPath = args.path;

		const newProject = createMockProject(
			this.projectId,
			`Project ${this.projects.length + 1}`,
			projectPath
		);
		this.projects = [...this.projects, newProject];

		return newProject;
	}

	getProject(args: InvokeArgs | undefined): Project {
		if (!args || !isGetProjectArgs(args)) {
			throw new Error('Invalid arguments for getting a project');
		}
		const projectId = args.projectId;
		const project = this.projects.find((p) => p.id === projectId);
		if (!project) {
			throw new Error(`Project with ID ${projectId} not found`);
		}
		return project;
	}

	getBaseBranchData(args: InvokeArgs | undefined) {
		if (!args || !isGetBaseBranchArgs(args)) {
			throw new Error('Invalid arguments for getting base branch data');
		}

		const projectId = args.projectId;

		return this.baseBranchData.get(projectId) ?? null;
	}

	setBaseBranch(args: InvokeArgs | undefined) {
		if (!args || !isSetBaseBranchArgs(args)) {
			throw new Error('Invalid arguments for setting base branch data');
		}

		const projectId = args.projectId;

		const baseBranch = mockBaseBranchData({
			branchName: args.branch
		});

		this.baseBranchData.set(projectId, baseBranch);
	}
}
