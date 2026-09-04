<script lang="ts">
	let loadingText = $state("CONNECTING TO QATEEL...");
</script>

<div class="flex flex-col gap-y-16 items-center justify-center">
	<div class="block relative w-60 h-fit">
		<div class="pac-man"></div>
	</div>
	<div class="block relative">
		<div class="loading-text">
			{#each loadingText.split('') as letter, index (index)}
				<span class="loading-text-words mx-[1.5px] xs:mx-[2px] md:mx-[5px]">{letter}</span>
			{/each}
		</div>
	</div>
</div>

<style>
	.pac-man {
		border-radius: 50%;
		border-radius: 100em 100em 0 0;
		background: var(--color-background);
		transform-origin: bottom;
		-webkit-animation: eating-top 0.5s infinite;
						animation: eating-top 0.5s infinite;
	}
	.pac-man,
	.pac-man::before {
		width: 70px;
		height: calc(70px/2);
		background: #e7bde7;
	}
	.pac-man::before {
		content: '';
		display: block;
		margin-top: calc(70px/2);
		position: absolute;
		transform-origin: top;
		border-radius: 0 0 100em 100em;
		transform: rotate(80deg);
		-webkit-animation: eating-bottom 0.5s infinite;
						animation: eating-bottom 0.5s infinite;
	}
	.pac-man::after {
		position: absolute;
		border-radius: 100em;
		content: '';
		display: block;
		height: 20px;
		width: 20px;
		margin-top: calc((70px / 2) - 10px);
		margin-left: calc((70px / 2) - 10px);
		transform-origin: center;
		-webkit-animation: center 0.5s infinite, ball 0.5s -0.33s infinite linear;
						animation: center 0.5s infinite, ball 0.5s -0.33s infinite linear;
	}
	@-webkit-keyframes eating-top {
		0% {
			transform: rotate(-40deg);
		}
		50% {
			transform: rotate(0deg);
		}
		100% {
			transform: rotate(-40deg);
		}
	}
	@keyframes eating-top {
		0% {
			transform: rotate(-40deg);
		}
		50% {
			transform: rotate(0deg);
		}
		100% {
			transform: rotate(-40deg);
		}
	}
	@-webkit-keyframes eating-bottom {
		0% {
			transform: rotate(80deg);
		}
		50% {
			transform: rotate(0deg);
		}
		100% {
			transform: rotate(80deg);
		}
	}
	@keyframes eating-bottom {
		0% {
			transform: rotate(80deg);
		}
		50% {
			transform: rotate(0deg);
		}
		100% {
			transform: rotate(80deg);
		}
	}
	@-webkit-keyframes center {
		0% {
			transform: rotate(40deg);
		}
		50% {
			transform: rotate(0deg);
		}
		100% {
			transform: rotate(40deg);
		}
	}
	@keyframes center {
		0% {
			transform: rotate(40deg);
		}
		50% {
			transform: rotate(0deg);
		}
		100% {
			transform: rotate(40deg);
		}
	}
	@-webkit-keyframes ball {
		0% {
			opacity: 0.7;
			box-shadow: 70px 0 0 0 #e7bde7, 120px 0 0 0 #e7bde7, 170px 0 0 0 #e7bde7, 220px 0 0 0 #e7bde7;
		}
		100% {
			box-shadow: 20px 0 0 0 #e7bde7, 70px 0 0 0 #e7bde7, 120px 0 0 0 #e7bde7, 170px 0 0 0 #e7bde7;
		}
	}
	@keyframes ball {
		0% {
			opacity: 0.7;
			box-shadow: 70px 0 0 0 #e7bde7, 120px 0 0 0 #e7bde7, 170px 0 0 0 #e7bde7, 220px 0 0 0 #e7bde7;
		}
		100% {
			box-shadow: 20px 0 0 0 #e7bde7, 70px 0 0 0 #e7bde7, 120px 0 0 0 #e7bde7, 170px 0 0 0 #e7bde7;
		}
	}

	.loading-text {
		text-align: center;
		width: 100%;
	}
	.loading-text span {
		display: inline-block;
		/* margin: 0 5px; */
		color: var(--color-muted-foreground);
	}
	.loading-text span:nth-child(1) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 0s infinite linear alternate;
						animation: blur-text 1.5s 0s infinite linear alternate;
	}
	.loading-text span:nth-child(2) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 0.2s infinite linear alternate;
						animation: blur-text 1.5s 0.2s infinite linear alternate;
	}
	.loading-text span:nth-child(3) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 0.4s infinite linear alternate;
						animation: blur-text 1.5s 0.4s infinite linear alternate;
	}
	.loading-text span:nth-child(4) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 0.6s infinite linear alternate;
						animation: blur-text 1.5s 0.6s infinite linear alternate;
	}
	.loading-text span:nth-child(5) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 0.8s infinite linear alternate;
						animation: blur-text 1.5s 0.8s infinite linear alternate;
	}
	.loading-text span:nth-child(6) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 1s infinite linear alternate;
						animation: blur-text 1.5s 1s infinite linear alternate;
	}
	.loading-text span:nth-child(7) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 1.2s infinite linear alternate;
						animation: blur-text 1.5s 1.2s infinite linear alternate;
	}
	.loading-text span:nth-child(8) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 1.2s infinite linear alternate;
						animation: blur-text 1.5s 1.2s infinite linear alternate;
	}
	.loading-text span:nth-child(9) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 1.2s infinite linear alternate;
						animation: blur-text 1.5s 1.2s infinite linear alternate;
	}
	.loading-text span:nth-child(10) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 1.2s infinite linear alternate;
						animation: blur-text 1.5s 1.2s infinite linear alternate;
	}
	.loading-text span:nth-child(11) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 1.2s infinite linear alternate;
						animation: blur-text 1.5s 1.2s infinite linear alternate;
	}
	.loading-text span:nth-child(12) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 1.2s infinite linear alternate;
						animation: blur-text 1.5s 1.2s infinite linear alternate;
	}
	.loading-text span:nth-child(13) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 1.2s infinite linear alternate;
						animation: blur-text 1.5s 1.2s infinite linear alternate;
	}
	.loading-text span:nth-child(15) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 1.2s infinite linear alternate;
						animation: blur-text 1.5s 1.2s infinite linear alternate;
	}
	.loading-text span:nth-child(16) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 1.2s infinite linear alternate;
						animation: blur-text 1.5s 1.2s infinite linear alternate;
	}
	.loading-text span:nth-child(17) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 1.2s infinite linear alternate;
						animation: blur-text 1.5s 1.2s infinite linear alternate;
	}
	.loading-text span:nth-child(18) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 1.2s infinite linear alternate;
						animation: blur-text 1.5s 1.2s infinite linear alternate;
	}
	.loading-text span:nth-child(19) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 1.2s infinite linear alternate;
						animation: blur-text 1.5s 1.2s infinite linear alternate;
	}
	.loading-text span:nth-child(20) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 1.2s infinite linear alternate;
						animation: blur-text 1.5s 1.2s infinite linear alternate;
	}
	.loading-text span:nth-child(21) {
		filter: blur(0px);
		-webkit-animation: blur-text 1.5s 1.2s infinite linear alternate;
						animation: blur-text 1.5s 1.2s infinite linear alternate;
	}

	@-webkit-keyframes blur-text {
		0% {
			filter: blur(0px);
		}
		100% {
			filter: blur(4px);
		}
	}

	@keyframes blur-text {
		0% {
			filter: blur(0px);
		}
		100% {
			filter: blur(4px);
		}
	}
</style>

