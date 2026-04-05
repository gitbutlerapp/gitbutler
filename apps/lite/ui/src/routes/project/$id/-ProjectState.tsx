import { createContext, type Dispatch, FC, ReactNode, useReducer } from "react";
import {
	initialProjectState,
	type ProjectState,
	type ProjectStateAction,
	projectStateReducer,
} from "./-state/project.ts";

export const ProjectStateContext = createContext<
	[ProjectState, Dispatch<ProjectStateAction>] | null
>(null);

export const ProjectStateProvider: FC<{
	children: ReactNode;
}> = ({ children }) => {
	const projectState = useReducer(projectStateReducer, initialProjectState);

	return <ProjectStateContext value={projectState}>{children}</ProjectStateContext>;
};
