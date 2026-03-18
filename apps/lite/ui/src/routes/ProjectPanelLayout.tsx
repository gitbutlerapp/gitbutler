import { FC, ReactNode, useContext } from "react";
import { Group, Panel, Separator, useDefaultLayout } from "react-resizable-panels";
import { PreviewVisibleContext } from "#ui/contexts/PreviewVisibleContext.ts";
import { assert } from "#ui/routes/project-shared.tsx";
import sharedStyles from "./project-shared.module.css";

export const ProjectPanelLayout: FC<{
	projectId: string;
	children: ReactNode;
	preview: ReactNode | null;
}> = ({ children, projectId, preview }) => {
	const [previewVisible] = assert(useContext(PreviewVisibleContext));
	const showPreview = previewVisible && preview !== null;
	const { defaultLayout, onLayoutChanged } = useDefaultLayout({
		id: `project:${projectId}:layout`,
		panelIds: showPreview ? ["primary", "preview"] : ["primary"],
	});

	return (
		<Group
			className={sharedStyles.pageWithPreview}
			defaultLayout={defaultLayout}
			onLayoutChange={onLayoutChanged}
		>
			<Panel id="primary" minSize={500}>
				<div className={sharedStyles.primaryPane}>{children}</div>
			</Panel>
			{showPreview && (
				<>
					<Separator className={sharedStyles.previewResizeHandle} />
					<Panel id="preview" minSize={300} defaultSize="50%">
						<div className={sharedStyles.previewPane}>{preview}</div>
					</Panel>
				</>
			)}
		</Group>
	);
};
