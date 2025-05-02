import type { AIService } from '$lib/ai/service';

export type Dependencies = {
	aiService: AIService;
	projectId: string;
};

export type Action<Args extends string[] = string[]> = {
	name: Uppercase<string>;
	description: string;
	requiredArgs: Args;
	execute: (
		args: Record<Args[number], string>,
		dependencies: Dependencies
	) => Promise<ActionResult>;
};

export type ActionResult =
	| {
			status: 'stop-execution';
	  }
	| {
			status: 'success';
			message: string;
	  }
	| {
			status: 'failure';
			message: string;
	  };
