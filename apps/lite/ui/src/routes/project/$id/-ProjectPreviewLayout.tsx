import { Dialog } from "@base-ui/react";
import { Match } from "effect";
import { FC, ReactNode, use, useEffect, useEffectEvent, useState } from "react";
import { Group, Panel, Separator, useDefaultLayout } from "react-resizable-panels";
import useLocalStorageState from "use-local-storage-state";
import uiStyles from "#ui/ui.module.css";
import { usePreviewVisible } from "#ui/hooks/usePreviewVisible.ts";
import { ShortcutBarPortalContext } from "#ui/routes/-ShortcutBarContext.tsx";
import { getShortcutAction, globalShortcutBindings, shortcutKeys } from "#ui/shortcuts.ts";
import { isTypingTarget } from "./-shared.tsx";
import sharedStyles from "./-shared.module.css";

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
	const inheritedShortcutBarPortalNode = use(ShortcutBarPortalContext);
	const [dialogShortcutBarPortalNode, setDialogShortcutBarPortalNode] =
		useState<HTMLElement | null>(null);
	const { defaultLayout, onLayoutChanged } = useDefaultLayout({
		id: `project:${projectId}:layout`,
		panelIds: showPreviewPanel ? ["primary", "preview"] : ["primary"],
	});

	const handleKeyDown = useEffectEvent((event: KeyboardEvent) => {
		if (event.defaultPrevented) return;
		if (event.metaKey || event.ctrlKey || event.altKey) return;
		if (isTypingTarget(event.target)) return;

		const action = getShortcutAction(globalShortcutBindings, undefined, event);
		if (!action) return;

		event.preventDefault();

		Match.value(action).pipe(
			Match.tag("TogglePreview", () => setShowPreviewPanel((x) => !x)),
			Match.tag("ToggleFullscreenPreview", () => setShowPreviewFullscreen((x) => !x)),
			Match.exhaustive,
		);
	});

	useEffect(() => {
		window.addEventListener("keydown", handleKeyDown);

		return () => {
			window.removeEventListener("keydown", handleKeyDown);
		};
	}, []);

	return (
		<ShortcutBarPortalContext
			value={
				showPreviewFullscreen
					? (dialogShortcutBarPortalNode ?? inheritedShortcutBarPortalNode)
					: inheritedShortcutBarPortalNode
			}
		>
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
									className={uiStyles.button}
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
						<div className={sharedStyles.previewDialogBody}>
							<Dialog.Close className={uiStyles.button}>
								Close fullscreen ({shortcutKeys.toggleFullscreenPreview}/esc)
							</Dialog.Close>
							{preview}
						</div>
						<footer
							className={sharedStyles.previewDialogShortcutBar}
							ref={setDialogShortcutBarPortalNode}
						/>
					</Dialog.Popup>
				</Dialog.Portal>
			</Dialog.Root>
		</ShortcutBarPortalContext>
	);
};
