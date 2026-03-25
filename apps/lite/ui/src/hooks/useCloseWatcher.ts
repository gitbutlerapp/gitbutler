import { useEffect, useEffectEvent, useRef } from "react";

export const useCloseWatcher = (onClose: () => void): (() => void) => {
	const closeWatcherRef = useRef<CloseWatcher | null>(null);
	const handleClose = useEffectEvent(() => {
		onClose();
	});

	useEffect(() => {
		const closeWatcher = new CloseWatcher();
		closeWatcherRef.current = closeWatcher;
		closeWatcher.addEventListener("close", handleClose);
		return () => {
			closeWatcher.removeEventListener("close", handleClose);
			closeWatcher.destroy();
			closeWatcherRef.current = null;
		};
	}, []);

	return () => {
		closeWatcherRef.current?.requestClose();
	};
};
