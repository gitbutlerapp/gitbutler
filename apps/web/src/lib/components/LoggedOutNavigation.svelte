<script lang="ts">
	import { env } from '$env/dynamic/public';
	function login() {
		window.location.href = `${env.PUBLIC_APP_HOST}cloud/login`;
	}
</script>

<div class="navigation">
  <div class="main-links">
    <div class="link">
      <a href="/" class="main-nav" aria-label="main nav" title="Home">
        <svg xmlns="http://www.w3.org/2000/svg" width="23" height="24" viewBox="0 0 23 24" fill="none">
          <path d="M0 24V0L11.4819 10.5091L23 0V24L11.4819 13.5273L0 24Z" fill="black"/>
        </svg>
      </a>
    </div>
    <div class="link">
      <a class="nav-link nav-button" href="/downloads" aria-label="downloads" title="Downloads">
        Downloads
      </a>
    </div>
    <div class="link">
      <a class="nav-link nav-button" href="https://docs.gitbutler.com" aria-label="docs" title="Documentation">
        Docs
      </a>
    </div>
  </div>
  <button
    type="button"
    class="nav-link nav-button"
    onclick={() => { login(); }}
  >
    Log In
  </button>
</div>

<style>
  .navigation {
    display: flex;
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 0 16px;
    margin: 0 auto;
  }

  .main-nav {
    margin-top: 16px;
    margin-right: 30px;
  }

  .main-links {
    display: flex;
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    gap: 26px;
    width: fit-content;
  }

  .link {
    display: flex;
    flex-direction: row;
    align-items: center;
    justify-content: center;
    gap: 8px;
    margin-left: 16px;
  }

  .nav-link {
    margin-top: 16px;
  }

  .nav-button {
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-s);
    height: 40px;
    width: 40px;
    white-space: nowrap;
    margin-right: 20px;
  }
</style>