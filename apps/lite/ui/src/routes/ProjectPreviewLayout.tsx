import { Dialog } from "@base-ui/react";
import { FC, ReactNode, useEffect, useEffectEvent } from "react";
import { Group, Panel, Separator, useDefaultLayout } from "react-resizable-panels";
import useLocalStorageState from "use-local-storage-state";
import { usePreviewVisible } from "../hooks/usePreviewVisible";
import { shortcutKeys } from "./shortcuts.ts";
import sharedStyles from "./project-shared.module.css";

const isTypingTarget = (target: EventTarget | null) => {
	if (!(target instanceof HTMLElement)) return false;
	return (
		target.isContentEditable ||
		target instanceof HTMLInputElement ||
		target instanceof HTMLTextAreaElement
	);
};

export const ProjectPreviewLayout: FC<{
	projectId: string;
	children: ReactNode;
	preview: ReactNode | null;
}> = ({ children, projectId, preview }) => {
	const [showPreviewPanel, setShowPreviewPanel] = usePreviewVisible();
	const [showPreviewFullscreen, setShowPreviewFullscreen] = useLocalStorageState(
		`project:${projectId}:showPreviewFullscreen`,
		{ defaultValue: false },
	);
	const { defaultLayout, onLayoutChanged } = useDefaultLayout({
		id: `project:${projectId}:layout`,
		panelIds: showPreviewPanel ? ["primary", "preview"] : ["primary"],
	});

	const handleKeyDown = useEffectEvent((event: KeyboardEvent) => {
		if (event.defaultPrevented || event.repeat) return;
		if (event.metaKey || event.ctrlKey || event.altKey) return;
		if (isTypingTarget(event.target)) return;

		switch (event.key.toLowerCase()) {
			case shortcutKeys.togglePreview:
				event.preventDefault();
				setShowPreviewPanel((x) => !x);
				break;
			case shortcutKeys.toggleFullscreenPreview:
				event.preventDefault();
				setShowPreviewFullscreen((x) => !x);
				break;
		}
	});

	useEffect(() => {
		window.addEventListener("keydown", handleKeyDown);

		return () => {
			window.removeEventListener("keydown", handleKeyDown);
		};
	}, []);

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
						<Panel id="preview" minSize={300} defaultSize="30%">
							<div className={sharedStyles.previewPane}>
								<button
									type="button"
									className={sharedStyles.button}
									onClick={() => {
										setShowPreviewFullscreen(true);
									}}
								>
									Open fullscreen ({shortcutKeys.toggleFullscreenPreview})
								</button>
								{preview}
							</div>
						</Panel>
					</>
				)}
			</Group>
			<Dialog.Root open={showPreviewFullscreen} onOpenChange={setShowPreviewFullscreen}>
				<Dialog.Portal>
					<Dialog.Popup aria-label="Preview" className={sharedStyles.previewDialogPopup}>
						<Dialog.Close className={sharedStyles.button}>
							Close fullscreen ({shortcutKeys.toggleFullscreenPreview}/esc)
						</Dialog.Close>
						{preview}
					</Dialog.Popup>
				</Dialog.Portal>
			</Dialog.Root>
		</>
	);
};
