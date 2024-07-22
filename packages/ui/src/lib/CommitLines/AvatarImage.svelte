<script lang="ts">
	import { tooltip } from '$lib/utils/tooltip';

	interface Props {
		srcUrl: string;
		tooltipText: string;
		altText: string;
	}

	let isLoaded = $state(false);

	const { srcUrl, tooltipText, altText }: Props = $props();
</script>

<div class="image-wrapper">
	<!-- <img
		class="avatar"
		alt={altText}
		src={srcUrl}
		loading="lazy"
		onload={() => (isLoaded = true)}
		width="100"
		height="100"
		class:hidden={!isLoaded}
		use:tooltip={tooltipText}
	/> -->
	<img
		class="avatar"
		class:hidden={isLoaded}
		alt={altText}
		src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACAAAAASCAYAAAA6yNxSAAAJcklEQVR4AQCBAH7/ANnXrf/Y163/19is/9XZq//R2an/zdqn/8jbpv/C26X/u9qk/7PYpP+r1aX/odGn/5fMqv+Nxq3/gr+x/3e3tf9srrn/YqW8/1idv/9QlsH/SpDB/0aMwf9FisD/R4q9/0qNuv9PkbX/VJaw/1mbq/9eoKb/YqWi/2Sonv9mqpz/AIEAfv8A2Neu/9jXrv/W2K3/1Nms/9Haqv/M2qj/x9um/8Lbpf+72qX/s9il/6vWpv+i0qj/mM2q/43Grv+Dv7L/eLe2/22vuf9ipr3/WZ6//1CXwf9KkcL/Ro3B/0aLwP9HjL3/S466/1CStv9Wl7H/W52s/2Cip/9kpqL/Z6mf/2irnP8AgQB+/wDV17D/1dew/9TYr//S2a7/z9qs/8vbqv/G26j/wdun/7rbpv+z2ab/q9en/6LTqf+Zzqz/j8iv/4TBs/95urb/brK6/2Oqvf9aosD/UZvC/0uVw/9HksL/R5DB/0qQvv9Ok7v/VJa3/1qbsv9goK3/ZaWo/2qppP9trKD/b66e/wCBAH7/ANHXtP/R2LT/0Niz/87Zsf/L2q//yNut/8Tcq//A3Kr/utyp/7Pbqf+s2Kn/pNWr/5vQrf+Ry7H/hsW0/3u9uP9wtrz/Za6//1unwf9SocP/TJzE/0mYxP9Jl8L/TZfA/1OZvf9ZnLj/YKG0/2elr/9tqqr/cq2m/3awov94sqD/AIEAfv8Ay9i4/8vYuP/K2bf/ydq1/8fbs//F3LH/wt2v/77drf+53az/s9ys/63arP+l167/nNOw/5POs/+IyLb/fsK6/3K7vf9ntMH/Xa7D/1Soxf9Oo8b/S6DG/02exP9Rn8L/WKG//2Cku/9op7b/b6ux/3avrP98s6j/gLal/4K3ov8AgQB+/wDE2L3/xNi9/8TZvP/D2rr/wtu4/8Ddtv++3rT/u96x/7fesP+z3q//rdyv/6bZsf+e1rP/ldK1/4vMuf+Axrz/dcDA/2m6w/9ftMX/Vq/H/1CryP9OqMj/UKfG/1anxP9eqMH/Z6u9/3CuuP94srT/gLav/4a5q/+Ku6f/jb2l/wCBAH7/ALzZw/+82cL/vNrB/7zbv/+83L3/u927/7rfuP+437b/teC0/7Hfs/+s3rP/pty0/5/Ytf+W1bj/jNC7/4LLv/92xcL/a8DF/2G7yP9Ytsn/U7LK/1Gwyv9Ur8n/W6/H/2Sww/9usr//eLW7/4G4tv+Ju7H/j76t/5TAqv+Xwaj/AIEAfv8As9rI/7PayP+028b/tNzE/7Xdwv+13r//td+9/7Tguv+y4Lj/r+C3/6vftv+l3bf/n9q4/5bXu/+N077/gs7B/3fJxP9sxcf/YsDK/1q8zP9Vucz/VbfM/1m2y/9gtsn/arfG/3S4wv9/u73/iL24/5HAtP+YwrD/ncSs/6DFqv8AgQB+/wCq283/q9vN/6vcy/+s3Mn/rt7H/6/fxP+v4MH/r+C+/67gvP+s4Lr/qN+5/6Teuv+d27v/ldi9/4zVwP+C0cP/eM3H/23Iyv9jxMz/XMHO/1i+z/9YvM7/XbvN/2W7y/9vvMj/er3E/4W/v/+Pwbr/mMS1/5/Fsf+kx67/p8is/wCBAH7/AKHc0v+i3NH/o93Q/6Tdzv+m3sv/qN/I/6ngxf+q4ML/qeC//6jgvf+l37z/oN28/5rbvv+T2MD/itXC/4HSxf93zsn/bcvM/2THzv9dxND/WsLR/1zA0P9hv8//ar/N/3TAyf+AwcX/i8LB/5XEvP+dxrf/pcey/6rIr/+tya3/AIEAfv8Amd3W/5re1f+b3tT/nd7S/5/fz/+h38v/ot/I/6Pfxf+j38L/ot7A/6Ddv/+c3L//ltrA/4/Xwv+H1cT/ftLH/3XPyv9rzM3/ZMnQ/17G0v9cxNL/X8PS/2XC0f9uwc7/ecLL/4TCxv+PxMH/mcW8/6LGt/+px7P/rsiv/7HJrf8AgQB+/wCS39n/kt/Z/5Pf1/+V39X/l9/S/5rfzv+b38v/nN7H/53exP+c3cL/mdvB/5Xawf+Q2MH/itXD/4LTxv960Mn/cs7M/2nLz/9jydH/X8bT/1/F1P9iw9P/acPS/3LCz/99wsz/iMPH/5LDwv+cxLz/pMW3/6vGsv+wx67/s8es/wCBAH7/AIzg3P+M4Nv/jeDa/4/g1/+R39T/k9/R/5Tezf+V3cn/ldzG/5TaxP+S2ML/jtbC/4nUw/+D0sX/fNDH/3XOyv9uy83/Z8nQ/2LH0/9fxtT/YcTV/2XD1P9swtP/dsLQ/4DCzP+Lwsf/lcLC/57CvP+mw7f/rcOx/7HErf+0xKv/AIEAfv8Ah+Le/4fi3f+I4dz/ieHZ/4vg1v+M3tP/jd3P/43by/+N2cj/jNfF/4nVw/+G08P/gdHE/3zOxv92zMj/b8rL/2nIzv9kx9H/YMXU/2DE1f9iwtb/aMLV/2/B0/95wND/g8DM/43Ax/+WwML/n8C7/6fAtf+twLD/scCs/7TAqf8AgQB+/wCD4+D/g+Pf/4Ti3f+F4dv/heDY/4be1P+G3ND/htnM/4XXyP+E1Mb/gdLE/33PxP95zcT/dMrG/27Iyf9pxsz/ZMXP/2DD0v9fwtT/YMHW/2TA1/9qv9b/cr/U/3u+0f+Fvsz/jr3H/5e9wf+gvLv/p7y0/6y8r/+xvKr/s7yn/wCBAH7/AIHk4f+B5OD/gePf/4Hi3P+B4Nn/gd3V/4Hb0f+A2M3/ftXJ/3zSxv95zsX/dczE/3HJxf9sx8f/Z8XJ/2PDzP9fwdD/XcDT/12/1f9gvtf/Zb7X/2u91v90vdT/fbzR/4a7zP+Pu8f/mLrA/6C5uv+muLP/rLit/7C3qP+yt6X/AIEAfv8Af+Xi/3/l4f9/5N//f+Ld/37g2f993dX/fNrR/3rWzf9408n/dc/H/3LMxf9uycT/asbF/2XDx/9hwcr/XcDN/1u+0P9avdP/XL3V/2C81/9lvNf/bbvW/3W71P9+utH/h7nM/5C4xv+Yt8D/n7a5/6a1sv+rtaz/rrSn/7G0o/8BgQB+/wB+5uL/fuXh/37k4P994t3/fODa/3vd1v952dH/d9bN/3TSyv9xzsf/bcrF/2nHxf9lxMX/YMHH/12/yv9avs3/WLzQ/1m80/9bu9b/YLvX/2a61/9uutb/drnU/3+40f+IuMz/kLfG/5i1v/+ftLj/pbOx/6qyq/+usqb/sLGi/+et3p4B1/E6AAAAAElFTkSuQmCC"
		width="100"
		height="100"
		use:tooltip={tooltipText}
	/>
</div>

<style lang="postcss">
	.image-wrapper {
		display: grid;
		place-content: center;
		overflow: hidden;
	}

	.image-wrapper > * {
		grid-area: 1 / 1;
	}

	.hidden {
		opacity: 0;
	}

	.avatar {
		position: relative;
		width: 12px;
		height: 12px;
		border-radius: 6px;
	}
</style>
