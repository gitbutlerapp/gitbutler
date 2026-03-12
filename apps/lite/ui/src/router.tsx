import { Outlet, createRootRoute, createRoute, createRouter } from "@tanstack/react-router";
import {  useQueryClient } from "@tanstack/react-query";
import type { ProjectForFrontend } from "@gitbutler/but-sdk";
import {  useState } from "react";
import { useTaskMutations, useTasks } from "@/hooks";


function RootLayout(): React.JSX.Element {
	return (
		<main style={{ fontFamily: "system-ui", margin: "2rem" }}>
			<h1>GitButler Lite</h1>
			<Outlet />
		</main>
	);
}

const rootRoute = createRootRoute({
	component: RootLayout,
});

const indexRoute = createRoute({
	getParentRoute: () => rootRoute,
	path: "/",
	component: HomePage,
	loader: async () => {
		const projects = await window.lite.listProjects();
		return { projects };
	},
});

function HomePage(): React.JSX.Element {
	const { projects } = indexRoute.useLoaderData();
	const [durationMsInput, setDurationMsInput] = useState("5000");
	const [createError, setCreateError] = useState<string | null>(null);
	const queryClient = useQueryClient();

	const tasksQuery = useTasks(queryClient);

	const { startTaskMutation, cancelTaskMutation } = useTaskMutations();

	/**
	 * Validates the duration input and creates a new long-running task via IPC.
	 */
	async function handleCreateTask(): Promise<void> {
		const durationMs = toDuration(durationMsInput);
		if (durationMs === null) {
			setCreateError("Duration must be an integer between 1 and 600000 ms.");
			return;
		}

		setCreateError(null);

		try {
			await startTaskMutation.mutateAsync(durationMs);
		} catch (error) {
			setCreateError(error instanceof Error ? error.message : "Failed to start task.");
		}
	}

	/**
	 * Requests cancellation for a task by id and refreshes snapshot state for the list.
	 */
	async function handleCancelTask(taskId: number): Promise<void> {
		try {
			const cancelled = await cancelTaskMutation.mutateAsync(taskId);
			if (!cancelled) {
				setCreateError("Task could not be cancelled (already finished).");
			}
		} catch (error) {
			setCreateError(error instanceof Error ? error.message : "Failed to cancel task.");
		}
	}

	const tasks = tasksQuery.data;

	return (
		<section>
			<p>Electron + Vite + TanStack Router scaffold is ready.</p>
			<h2>Projects list</h2>
			<ProjectsList projects={projects} />

			<h2>Long-running tasks</h2>
			<div style={{ display: "flex", gap: "0.5rem", marginBottom: "0.75rem" }}>
				<input
					type="number"
					min={1}
					max={600000}
					step={1}
					value={durationMsInput}
					onChange={(event) => {
						setDurationMsInput(event.target.value);
					}}
					placeholder="Duration in ms"
				/>
				<button
					type="button"
					onClick={() => void handleCreateTask()}
					disabled={startTaskMutation.isPending}
				>
					Create task
				</button>
			</div>

			{createError ? <p style={{ color: "crimson" }}>{createError}</p> : null}
			{tasksQuery.isError ? <p style={{ color: "crimson" }}>Failed to load task snapshots.</p> : null}

			{tasks.length === 0 ? (
				<p>No tasks started yet.</p>
			) : (
				<div>
					{tasks.map((task) => (
						<div
							key={task.taskId}
							style={{
								border: "1px solid #d9d9d9",
								padding: "0.75rem",
								marginBottom: "0.5rem",
								borderRadius: "0.375rem",
							}}
						>
							<p>
								<strong>Task #{task.taskId}</strong>
							</p>
							<p>Duration: {task.durationMs} ms</p>
							<p>Step: {task.step}</p>
							<p>Status: {task.status}</p>
							{task.message ? <p style={{ color: "crimson" }}>{task.message}</p> : null}
							<button
								type="button"
								onClick={() => {
									void handleCancelTask(task.taskId);
								}}
								disabled={task.status !== "running" || cancelTaskMutation.isPending}
							>
								Cancel
							</button>
						</div>
					))}
				</div>
			)}
		</section>
	);
}

interface ProjectsListProps {
	projects: ProjectForFrontend[];
}

const routeTree = rootRoute.addChildren([indexRoute]);
function toDuration(durationMsInput: string): number | null {
	if (!/^\d+$/.test(durationMsInput)) {
		return null;
	}

	const durationMs = Number(durationMsInput);
	if (!Number.isInteger(durationMs)) {
		return null;
	}

	if (durationMs < 1 || durationMs > 600000) {
		return null;
	}

	return durationMs;
}


export const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
	interface Register {
		router: typeof router;
	}
}

function ProjectsList(props: ProjectsListProps) {
	if (props.projects.length === 0) {
		return <p> no projects :(</p>;
	}
	return (
		<div>
			{props.projects.map((project) => (
				<p key={project.id}>{project.title}</p>
			))}
		</div>
	);
}