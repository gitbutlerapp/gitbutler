import { Dialog } from "@base-ui/react";
import { FC, ReactNode, useContext } from "react";
import { Group, Panel, Separator, useDefaultLayout } from "react-resizable-panels";
import { ShowPreviewPanelContext } from "#ui/contexts/ShowPreviewPanelContext.ts";
import { useLocalStorageState } from "#ui/hooks/useLocalStorageState.ts";
import { assert } from "#ui/routes/project-shared.tsx";
import sharedStyles from "./project-shared.module.css";

export const ProjectPanelLayout: FC<{
	projectId: string;
	children: ReactNode;
	preview: ReactNode | null;
}> = ({ children, projectId, preview }) => {
	const [_showPreviewPanel] = assert(useContext(ShowPreviewPanelContext));
	const [_showPreviewFullscreen, setShowPreviewFullscreen] = useLocalStorageState(
		`project:${projectId}:showPreviewFullscreen`,
		false,
	);
	const showPreviewPanel = _showPreviewPanel && preview !== null;
	const showPreviewFullscreen = _showPreviewFullscreen && preview !== null;
	const { defaultLayout, onLayoutChanged } = useDefaultLayout({
		id: `project:${projectId}:layout`,
		panelIds: showPreviewPanel ? ["primary", "preview"] : ["primary"],
	});

	return (
		<>
			<Group
				className={sharedStyles.pageWithPreview}
				defaultLayout={defaultLayout}
				onLayoutChange={onLayoutChanged}
			>
				<Panel id="primary" minSize={500}>
					<div className={sharedStyles.primaryPane}>{children}</div>
				</Panel>
				{showPreviewPanel && (
					<>
						<Separator className={sharedStyles.previewResizeHandle} />
						<Panel id="preview" minSize={300} defaultSize="50%">
							<div className={sharedStyles.previewPane}>
								<button
									type="button"
									onClick={() => {
										setShowPreviewFullscreen(true);
									}}
								>
									Open fullscreen
								</button>
								{preview}
							</div>
						</Panel>
					</>
				)}
			</Group>
			{preview !== null && (
				<Dialog.Root open={showPreviewFullscreen} onOpenChange={setShowPreviewFullscreen}>
					<Dialog.Portal>
						<Dialog.Popup aria-label="Preview" className={sharedStyles.previewDialogPopup}>
							<Dialog.Close>Close fullscreen</Dialog.Close>
							{preview}
						</Dialog.Popup>
					</Dialog.Portal>
				</Dialog.Root>
			)}
		</>
	);
};
