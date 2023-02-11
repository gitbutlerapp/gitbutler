<script>
  let response = null;
  let interval = null;
  let token = null;

  let login;
  let user;

  async function postData(url = "", data = {}) {
    const response = await fetch(url, {
      method: "POST", // *GET, POST, PUT, DELETE, etc.
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(data), // body data type must match "Content-Type" header
    });
    return response.json(); // parses JSON response into native JavaScript objects
  }

  const fetchData = async (token) => {
    try {
      const res = await fetch(
        "https://chacons.eu.ngrok.io/api/login/user/" + token
      );
      response = res.status;
      console.log(response);
      if (response === 200) {
        console.log(res);
        user = await res.json();
        console.log(user);
        clearInterval(interval);
      }
    } catch (error) {
      console.error(error);
    }
  };

  const logIn = () => {
    postData("https://chacons.eu.ngrok.io/api/login/token")
      .then((response) => {
        console.log(response);
        token = response.token;
        console.log(token);
        let url = "https://chacons.eu.ngrok.io/login/" + token;
        login.href = url;
        interval = setInterval(() => {
          fetchData(token);
        }, 5000);
      })
      .then((data) => console.log(data));
  };

  logIn();
</script>

<h1>Your User Account</h1>
<hr />

Temp Token: {token}

<hr />

<a bind:this={login} target="_blank" href="#"> Log In </a><br />
{#if user}
  User Data: {JSON.stringify(user)}
{/if}
