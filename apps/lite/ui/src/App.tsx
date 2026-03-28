import { Toast, ToastManager, Tooltip } from "@base-ui/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { RegisteredRouter, RouterProvider } from "@tanstack/react-router";
import { FC, StrictMode } from "react";
import styles from "./App.module.css";

const Toasts: FC = () => {
	const { toasts } = Toast.useToastManager();

	return (
		<Toast.Portal>
			<Toast.Viewport className={styles.toastViewport}>
				{toasts.map((toast) => (
					<Toast.Root key={toast.id} toast={toast} className={styles.toastRoot}>
						<Toast.Content>
							<Toast.Title />
							<Toast.Description
								render={
									// Default is `p` which restricts content elements.
									<div />
								}
							/>
							<Toast.Close>Dismiss</Toast.Close>
						</Toast.Content>
					</Toast.Root>
				))}
			</Toast.Viewport>
		</Toast.Portal>
	);
};
export const App: React.FC<{
	queryClient: QueryClient;
	toastManager: ToastManager;
	router: RegisteredRouter;
}> = ({ queryClient, toastManager, router }) => (
	<StrictMode>
		<QueryClientProvider client={queryClient}>
			<Toast.Provider toastManager={toastManager}>
				<Tooltip.Provider>
					<RouterProvider router={router} />
					<Toasts />
				</Tooltip.Provider>
			</Toast.Provider>
			<ReactQueryDevtools />
		</QueryClientProvider>
	</StrictMode>
);
