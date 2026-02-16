import type { Toast } from '$lib/notifications/toasts';

export function mapErrorToToast(err: any): Toast | undefined {
	// We expect an object to be thrown by octokit.
	if (typeof err !== 'object') return;

	const status = err?.status;
	const response = err?.response;
	const data = response?.data;
	const message = data?.message;
	const errors = data?.errors;

	// If this expectation isn't met we must be doing something wrong
	if (status === undefined || message === undefined) return;

	if (message.includes('Draft pull requests are not supported')) {
		return {
			title: 'Draft pull requests are not enabled',
			message: `
                It looks like draft pull requests are not enabled in your repository.

                Please see our [documentation](https://docs.gitbutler.com/)
                for additional help.
            `,
			error: message,
			style: 'danger'
		};
	}

	if (message.includes('enabled OAuth App access restrictions')) {
		return {
			title: 'OAuth access restricted',
			message: `
				It looks like OAuth access has been restricted by your organization.

				Please see our [documentation](https://docs.gitbutler.com/)
				for additional help.
			`,
			error: message,
			style: 'danger'
		};
	}
	if (message.includes('Validation Failed')) {
		let errorStrings = '';
		if (errors instanceof Array) {
			errorStrings = errors
				.map((err) => {
					if (err.message) return err.message;
					if (err.field && err.code) return `${err.field} ${err.code}`;
					return 'unknown validation error';
				})
				.join('\n');
		}
		return {
			title: 'GitHub validation failed',
			message: `
                It seems there was a problem validating the request.

                Please see our [documentation](https://docs.gitbutler.com/)
                for additional help.
            `,
			error: errorStrings,
			style: 'danger'
		};
	}
}
