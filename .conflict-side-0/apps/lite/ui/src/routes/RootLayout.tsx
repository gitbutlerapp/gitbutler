import { HotkeysProvider } from "@tanstack/react-hotkeys";
import { Outlet } from "@tanstack/react-router";
import { FC } from "react";
import styles from "./RootLayout.module.css";

export const RootLayout: FC = () => (
	<HotkeysProvider>
		<div className={styles.dragRegion} />
		<main className={styles.content}>
			<Outlet />
		</main>
	</HotkeysProvider>
);
