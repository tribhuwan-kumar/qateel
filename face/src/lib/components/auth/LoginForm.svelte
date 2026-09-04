<script lang="ts">
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
	import { quintOut } from "svelte/easing";
  import { checkAuth } from "$lib/auth/auth.svelte";
	import { isValidPassword, isValidUsername } from "$lib/utils";
  import { fly, fade, slide } from 'svelte/transition';
  import { Button } from "$lib/components/ui/button";
  import { Label } from "$lib/components/ui/label";
  import { HugeiconsIcon } from "@hugeicons/svelte";
	import { LoaderPinwheelIcon } from "@hugeicons/core-free-icons";
	import { secureFetch } from "$lib/utils";
  import {
    CardHeader,
    CardTitle,
    CardDescription,
    CardContent,
    CardFooter
  } from "$lib/components/ui/card";

	let currentView = $state<"login" | "request-token" | "reset-password">("login");

	/* Login related */
  let loginUsername = $state("");
  let loginPassword = $state("");
  let loading = $state(false);
  let error = $state("");

	/* Reset pass related */
	let resetUsername = $state("");
  let resetToken = $state("");
  let resetPassStep = $state(1);
  let newPassword = $state("");
  let confirmNewPassword = $state("");
  let resetLoading = $state(false);
  let resetError = $state("");

  async function handleLogin(event: SubmitEvent) {
		event.preventDefault();
    loading = true;
    error = "";

    if (loginUsername.length < 6) {
      error = "Username must be at least 6 characters!!";
      loading = false;
      toast.error(error, {
        richColors: true,
        style: "cursor: pointer;"
      })
      return;
    }

    if (!isValidUsername(loginUsername)) {
      error = "Username can only contain letters, numbers, and underscores!!";
      loading = false;
      toast.error(error, {
        richColors: true,
        style: "cursor: pointer;"
      })
      return;
    }

    try {
      const res = await secureFetch("/api/auth/login", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ username: loginUsername, password: loginPassword })
      });

      const data = await res.json();
      console.log("loging data", data)
      if (!res.ok) throw new Error(data.error || "Login failed");

      if (res.ok) {
        const isLoggedIn = await checkAuth();
        if (isLoggedIn) {
          goto("/dashboard");
        }
      }
    } catch (e: any) {
      error = e.message;
      toast.error("Auth error:", {
        description: e.message,
        richColors: true,
        style: "cursor: pointer;"
      })
    } finally {
      loading = false;
    }
  }

	async function handleRequestToken(event: SubmitEvent) {
    event.preventDefault();
    resetLoading = true;
    resetError = "";

    if (resetUsername.length < 6) {
      resetError= "Username must be at least 6 characters!!";
      resetLoading= false;
      toast.error(resetError, {
        richColors: true,
        style: "cursor: pointer;"
      })
			resetLoading = false;
      return;
    }

    try {
      const res = await secureFetch("/api/auth/admin/gen_password_reset_token", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ username: resetUsername })
      });

      const data = await res.json();

      if (!res.ok) {
        throw new Error(data.error || "Failed to generate token");
      }

      toast.info(`Password reset token generated for '${resetUsername}'`, {
				description: "Please check your Qateel's data directory on server!!",
				richColors: true,
        style: "cursor: pointer;"
			});
      currentView = "reset-password";

    } catch (e: any) {
      resetError = e.message;
      toast.error("Token generation error:", {
				description: e.message,
				richColors: true,
        style: "cursor: pointer;"
			});
    } finally {
      resetLoading = false;
    }
  }

	async function handleResetPassword(event: SubmitEvent) {
    event.preventDefault();
    if (resetPassStep === 1) {
      if (!isValidPassword(newPassword)) {
        resetError = "Password must be at least 8 characters!!";
        resetLoading = false;
        toast.error(resetError, {
          richColors: true,
          style: "cursor: pointer;"
        })
        return;
      }
      resetPassStep= 2;
      return;
    }

		if (resetPassStep === 2) {
			if (newPassword !== confirmNewPassword) {
				resetError = "Passwords do not match!";
				newPassword = ""
				confirmNewPassword = ""
				resetPassStep = 1
        resetLoading= false;
        toast.error(resetError, {
          richColors: true,
          style: "cursor: pointer;"
        })
				return;
			}

			resetLoading = true;
			resetError = "";

			try {
				const res = await secureFetch("/api/auth/admin/reset_password", {
					method: "POST",
					headers: { "Content-Type": "application/json" },
					body: JSON.stringify({
						username: resetUsername,
						resetToken: resetToken.trim(),
						newPassword: newPassword
					})
				});

				const data = await res.json();

				if (!res.ok) {
					throw new Error(data.error || "Failed to reset password");
				}

				toast.success(`Successfully reset password for '${data.username}'`, {
					description: "You can now sign in!!",
					richColors: true,
					style: "cursor: pointer;"
				});

				resetToken = "";
				newPassword = "";
				loginPassword = "";
				confirmNewPassword = "";
				currentView = "login";

			} catch (e: any) {
				resetError = e.message;
				toast.error("Error in resetting password:", {
					description: e.message,
					richColors: true
				});
			} finally {
				resetLoading = false;
			}
		}
  }
</script>

<div class="relative grid overflow-hidden w-full min-h-[380px] place-items-stretch">
	{#if currentView === "login"}
		<div
			class="col-start-1 row-start-1 flex flex-col w-full"
      in:fly={{ y: 20, duration: 300, delay: 150, easing: quintOut }}
      out:fade={{ duration: 150 }}
    >
		<CardHeader>
			<CardTitle class="text-2xl font-bold text-center flex justify-center">
				Qateel
			</CardTitle>
			<CardDescription class="text-center">Enter your credentials to access the dashboard.</CardDescription>
		</CardHeader>

		<CardContent class="mr-auto! ml-auto! py-6">
			<form
				onsubmit={handleLogin}
				class="space-y-4 flex flex-col justify-center w-full"
			>
				<div class="space-y-2">
					<Label for="username" class="text-muted-foreground text-[14px] ml-1">Username</Label>
					<input
						id="username"
						type="text"
						autocomplete="off"
						autocorrect="off"
						autocapitalize="off"
						bind:value={loginUsername}
						disabled={loading}
						class="disabled:cursor-not-allowed! md:w-[98%] w-[98%] ml-[2px] rounded-2xl border-2 border-sidebar-border/60 bg-transparent px-2 py-1.5 outline-none placeholder:text-muted-foreground/50 text-foreground transition-shadow focus-visible:ring-2 focus-visible:ring-sidebar-border"
						required
					/>
				</div>

				<div class="space-y-2">
					<Label for="password" class="text-muted-foreground text-[14px] ml-1">Password</Label>
					<input
						id="password"
						type="password"
						autocomplete="off"
						autocorrect="off"
						autocapitalize="off"
						bind:value={loginPassword}
						disabled={loading}
						class="disabled:cursor-not-allowed! md:w-[98%] w-[98%] ml-[2px] rounded-2xl border-2 border-sidebar-border/60 bg-transparent
							px-2 py-1.5 outline-none placeholder:text-muted-foreground/50 text-foreground transition-shadow focus-visible:ring-2 focus-visible:ring-sidebar-border"
						required
					/>
				</div>

				<Button
					type="submit"
					class="mt-2 cursor-pointer disabled:cursor-not-allowed!"
					disabled={loading}
				>
					{#if loading}
						<HugeiconsIcon
							icon={LoaderPinwheelIcon}
							class="w-4 h-4 mr-2"
							strokeWidth={2}
							size={24}
						/>
						Wait a sec...
					{:else}
						Sign in
					{/if}
				</Button>
			</form>
		</CardContent>

		<CardFooter
			class="justify-center mt-auto"
		>
			<button
				onclick={() => currentView = "request-token"}
				class="text-[14px] text-muted-foreground cursor-pointer ml-1 text-center relative
					before:absolute before:bottom-0 before:left-0 before:h-[2px] before:w-full before:origin-right
					before:scale-x-0 before:bg-muted-foreground/60 before:rounded-full before:transition-transform before:duration-300
					before:ease-in-out hover:before:origin-left hover:before:scale-x-100"
				>
				Forgot password!?
			</button>
		</CardFooter>
	</div>

	{:else if currentView === "request-token"}
    <div
			class="col-start-1 row-start-1 flex flex-col w-full"
			in:fly={{ y: 20, duration: 300, delay: 150, easing: quintOut }}
      out:fade={{ duration: 150 }}
    >
      <CardHeader>
        <CardTitle class="text-xl font-bold text-center">Account recovery</CardTitle>
        <CardDescription class="text-center">Enter username to generate a password reset token file on the server</CardDescription>
      </CardHeader>

      <CardContent class="mr-auto! ml-auto! py-6">
        <form onsubmit={handleRequestToken} class="space-y-6 flex flex-col justify-center w-full mt-4">
          <div class="space-y-2">
            <Label for="resetUsername"
							class="text-muted-foreground text-[14px] ml-1">
							Username
						</Label>
            <input
              type="text"
							autocomplete="off"
							autocorrect="off"
							autocapitalize="off"
							id="resetUsername"
              bind:value={resetUsername}
              disabled={resetLoading}
              class="disabled:cursor-not-allowed! md:w-[98%] w-[98%] ml-[2px] rounded-2xl border-2 border-sidebar-border/60 bg-transparent
								px-2 py-1.5 outline-none placeholder:text-muted-foreground/50 text-foreground transition-shadow focus-visible:ring-2 focus-visible:ring-sidebar-border"
              required
            />
          </div>

          <Button type="submit" class="mt-4 cursor-pointer disabled:cursor-not-allowed!" disabled={resetLoading}>
            {#if resetLoading}
              <HugeiconsIcon icon={LoaderPinwheelIcon} class="w-4 h-4 mr-2 animate-spin" strokeWidth={2} />
              Generating...
            {:else}
              Request token
            {/if}
          </Button>
        </form>
      </CardContent>

      <CardFooter class="justify-center mt-auto">
        <button
					type="button"
					onclick={() => {
						currentView = "login";
						resetError = "";
					}}
					class="text-[14px] text-muted-foreground cursor-pointer ml-1 text-center relative
						before:absolute before:bottom-0 before:left-0 before:h-[2px] before:w-full before:origin-right
						before:scale-x-0 before:bg-muted-foreground/60 before:rounded-full before:transition-transform before:duration-300
						before:ease-in-out hover:before:origin-left hover:before:scale-x-100"
					>
          Back to sign in
        </button>
      </CardFooter>
    </div>
  {:else if currentView === "reset-password"}
    <div
			class="col-start-1 row-start-1 flex flex-col w-full"
			in:fly={{ y: 20, duration: 300, delay: 150, easing: quintOut }}
      out:fade={{ duration: 150 }}
    >
      <CardHeader>
        <CardTitle
					class="text-xl font-bold text-center"
				>
					Verify token
				</CardTitle>
        <CardDescription
					class="text-center"
				>
						Paste the raw token from the generated file to set a new password.
				</CardDescription>
      </CardHeader>

      <CardContent class="mr-auto! ml-auto! py-6 w-85">
        <form onsubmit={handleResetPassword} class="space-y-3 flex flex-col justify-center">
          <div class="space-y-1">
            <Label for="resetToken" class="text-muted-foreground text-[13px] ml-1">Secure token</Label>
            <input
              id="resetToken"
              type="text"
							autocomplete="off"
							autocorrect="off"
							autocapitalize="off"
              bind:value={resetToken}
              disabled={resetLoading || resetPassStep === 2}
              class="text-xs disabled:cursor-not-allowed! md:w-[98%] w-[98%] ml-[2px] rounded-2xl border-2
								border-sidebar-border/60 bg-transparent px-2 py-1.5 outline-none placeholder:text-muted-foreground/50
								text-foreground transition-shadow focus-visible:ring-2 focus-visible:ring-sidebar-border"
              required
            />
          </div>

					<div class="overflow-hidden h-16">
						{#if resetPassStep === 1}
							<div class="space-y-1"
								out:slide={{ axis: 'x', duration: 400 }}
								onoutroend={() => resetPassStep = 2}
							>
								<Label for="newPassword" class="text-muted-foreground text-[13px] ml-1">New password</Label>
								<input
									id="newPassword"
									type="password"
									autocomplete="off"
									autocorrect="off"
									autocapitalize="off"
									bind:value={newPassword}
									disabled={resetLoading}
									class="disabled:cursor-not-allowed! md:w-[98%] w-[98%] ml-[2px] rounded-2xl border-2
										border-sidebar-border/60 bg-transparent j px-2 py-1.5 outline-none
										placeholder:text-muted-foreground/50 text-foreground transition-shadow focus-visible:ring-2 focus-visible:ring-sidebar-border"
									required
								/>
							</div>
						{:else if resetPassStep === 2}
							<div
								class="space-y-1"
								in:slide={{ axis: 'x', duration: 600, delay: 500 }}
							>
								<Label for="confirmNewPassword" class="text-muted-foreground text-[13px] ml-1">Confirm Password</Label>
								<input
									id="confirmNewPassword"
									type="password"
									autocomplete="off"
									autocorrect="off"
									autocapitalize="off"
									bind:value={confirmNewPassword}
									disabled={resetLoading}
									class="disabled:cursor-not-allowed! md:w-[98%] w-[98%] ml-[2px] rounded-2xl border-2
										border-sidebar-border/60 bg-transparent px-2 py-1.5 outline-none placeholder:text-muted-foreground/50
										text-foreground transition-shadow focus-visible:ring-2 focus-visible:ring-sidebar-border"
									required
								/>
							</div>
						{/if}
					</div>

          <Button
						type="submit"
						class="mt-2 cursor-pointer disabled:cursor-not-allowed!"
						disabled={resetLoading}
					>
            {#if resetLoading}
              <HugeiconsIcon
								icon={LoaderPinwheelIcon}
								class="w-4 h-4 mr-2 animate-spin" strokeWidth={2}
							/>
              Resetting...
						{:else if resetPassStep === 1}
							Next
						{:else}
              Reset password
						{/if}
          </Button>
        </form>
      </CardContent>

      <CardFooter class="justify-center mt-auto">
        <button
					type="button"
					onclick={
						() => { currentView = "request-token";
						resetError = "";
					}}
					class="text-[14px] text-muted-foreground cursor-pointer ml-1 text-center relative
						before:absolute before:bottom-0 before:left-0 before:h-[2px] before:w-full before:origin-right
						before:scale-x-0 before:bg-muted-foreground/60 before:rounded-full before:transition-transform before:duration-300
						before:ease-in-out hover:before:origin-left hover:before:scale-x-100"
					>
          Back
        </button>
      </CardFooter>
    </div>
  {/if}
</div>
