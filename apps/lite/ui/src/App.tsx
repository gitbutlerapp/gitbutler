import { router } from "#ui/router.tsx";
import { Toast, ToastManager } from "@base-ui/react/toast";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { RouterProvider } from "@tanstack/react-router";
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
							<Toast.Description />
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
}> = ({ queryClient, toastManager }) => (
	<StrictMode>
		<QueryClientProvider client={queryClient}>
			<Toast.Provider toastManager={toastManager}>
				<RouterProvider router={router} />
				<Toasts />
			</Toast.Provider>
			<ReactQueryDevtools />
		</QueryClientProvider>
	</StrictMode>
);
