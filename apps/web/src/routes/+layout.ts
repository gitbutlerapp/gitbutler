import { AuthService } from '$lib/auth/authService';
import { UserService } from '$lib/user/userService';
import type { LayoutLoad } from './$types';

// eslint-disable-next-line
export const load: LayoutLoad = async ({ context }) => {
	const authService = new AuthService();
	const userService = new UserService();

	return {
		authService,
		userService
	};
};
