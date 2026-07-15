type Trigger<Arg, Data> = (arg: Arg) => { unwrap: () => Promise<Data> };

type MutationState<Data> = {
	data?: Data;
	error?: unknown;
	isError: boolean;
	isLoading: boolean;
	reset: () => void;
};

type MutationCallbacks<Data> = {
	onError?: (error: unknown) => void;
	onSuccess?: (data: Data) => void;
};

export const mutationResult = <Arg, Data>(
	trigger: Trigger<Arg, Data>,
	state: MutationState<Data>,
	lifecycle: {
		onError?: (error: unknown, input: Arg) => void | Promise<void>;
		onSuccess?: (data: Data, input: Arg) => void | Promise<void>;
	} = {},
) => {
	const mutateAsync = async (input: Arg): Promise<Data> => {
		try {
			const data = await trigger(input).unwrap();
			await lifecycle.onSuccess?.(data, input);
			return data;
		} catch (error) {
			await lifecycle.onError?.(error, input);
			throw error;
		}
	};

	return {
		...state,
		isPending: state.isLoading,
		mutate: (input: Arg, callbacks?: MutationCallbacks<Data>) => {
			void mutateAsync(input).then(
				(data) => callbacks?.onSuccess?.(data),
				(error) => callbacks?.onError?.(error),
			);
		},
		mutateAsync,
	};
};
