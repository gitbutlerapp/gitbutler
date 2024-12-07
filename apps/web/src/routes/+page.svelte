<!-- TODO: go to marketing page if we're not logged in -->

<svelte:head>
	<title>Home</title>
</svelte:head>

<section>
	<h1>Welcome to GitButler</h1>
	<p>GitButler is a platform for managing your repositories and projects.</p>
	<div>
		<a href="/login">Log In</a>
	</div>
</section>

<style>
	section {
		display: flex;
		flex-direction: column;
		justify-content: center;
		align-items: center;
		flex: 0.6;
	}

	h1 {
		width: 100%;
	}
</style>
