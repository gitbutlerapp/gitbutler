# Modern Layout Architecture: CSS Grid with Named Lines and Subgrids

## The Problem with Traditional Breakpoint Management

When building complex layouts, developers typically face a common challenge: managing breakpoints and column widths across multiple components. The traditional approach involves:

- Defining media queries in each component
- Managing breakpoint consistency across the entire application
- Duplicating layout logic for different viewport sizes
- Wrestling with nested responsive components that need to align with a global grid

This leads to fragmented CSS, maintenance headaches, and inconsistent spacing across your application.

## A Better Solution: Global Grid with Named Lines

At GitButler, we've implemented an elegant solution that eliminates these issues using CSS Grid's named lines and subgrids. Here's how it works:

### The Foundation: Parent Grid Container

The main layout container defines a comprehensive grid with named lines for different content areas:

```css
.marketing-page {
	display: grid;
	grid-template-columns:
		[full-start]
		1fr 1fr
		[narrow-start]
		1fr 1fr 1fr 1fr 1fr 1fr 1fr
		[narrow-end]
		1fr [off-gridded] 1fr
		[full-end];
	column-gap: var(--layout-col-gap);
	row-gap: 60px;
	width: 100%;
	max-width: 1440px;
	margin: 0 auto;
	padding: 0 var(--layout-side-paddings);
}
```

This creates three distinct content areas:

- **Full width**: `full-start` to `full-end` (entire container width)
- **Narrow content**: `narrow-start` to `narrow-end` (central content area)
- **Off-gridded**: Special positioning outside the main grid

### Responsive Grid Adaptation

The grid automatically adapts to different screen sizes using CSS custom properties and media queries:

```css
@media (--desktop-small-viewport) {
	grid-template-columns:
		[full-start]
		1fr
		[narrow-start]
		1fr 1fr 1fr 1fr 1fr 1fr 1fr 1fr 1fr
		[narrow-end off-gridded]
		1fr
		[full-end];
}

@media (--mobile-viewport) {
	grid-template-columns:
		[full-start narrow-start]
		1fr 1fr 1fr 1fr
		[narrow-end full-end off-gridded];
	row-gap: 40px;
	padding: 0 24px;
}
```

Notice how the named lines are repositioned rather than redefined. On mobile, `narrow-start` and `full-start` become the same line, automatically adjusting all child components.

### The Magic: Subgrid Implementation

Each section component uses `subgrid` to inherit the parent's column definitions:

```css
.hero {
	display: grid;
	grid-template-columns: subgrid;
	grid-column: full-start / full-end;
}

.hero-content {
	grid-column: narrow-start / narrow-end;
	max-width: 700px;
	padding-top: 52px;
}
```

```css
.ai-features {
	display: grid;
	grid-template-columns: subgrid;
	grid-column: full-start / full-end;
	border-radius: var(--radius-xl);
	background-color: var(--clr-bg-2);
}

.ai-features__video {
	grid-column: narrow-start / narrow-end;
	aspect-ratio: 16 / 9;
	width: 100%;
}
```

## Key Benefits

### 1. No Component-Level Breakpoints

Components don't need their own media queries. They simply declare which grid area they want to occupy:

```css
.section-header {
	grid-column: narrow-start / narrow-end;
}

.video-content {
	grid-column: full-start / full-end;
}
```

### 2. Automatic Responsive Behavior

When the parent grid changes its column definitions, all child components automatically adapt without any changes to their CSS.

### 3. Consistent Spacing

The `--layout-col-gap` variable ensures consistent spacing between grid items across all viewport sizes and components.

### 4. Flexible Content Areas

Components can easily switch between different layout areas:

- Use `narrow-start / narrow-end` for standard content width
- Use `full-start / full-end` for full-width sections like hero banners
- Use `off-gridded` for elements that need to break out of the main grid

## Global Layout Variables

The system relies on a few CSS custom properties that adapt to viewport size:

```css
:root {
	--layout-col-gap: 20px;
	--layout-side-paddings: 80px; /* Desktop */
}

@media (--desktop-small-viewport) {
	:root {
		--layout-side-paddings: 40px;
	}
}

@media (--mobile-viewport) {
	:root {
		--layout-side-paddings: 16px;
	}
}
```

## Real-World Implementation

Here's how different sections implement this pattern:

```html
<!-- Hero Section -->
<section class="hero">
	<div class="hero-content">
		<h1>Main Title</h1>
		<!-- Content automatically positioned in narrow area -->
	</div>
</section>

<!-- Full-width Feature Section -->
<section class="feature-updates">
	<div class="video-content">
		<!-- Full-width video content -->
	</div>
	<div class="feature-text">
		<!-- This could be narrow-width text content -->
	</div>
</section>

<!-- Standard Content Section -->
<section class="section-header">
	<h2>Section Title</h2>
	<!-- Automatically positioned in narrow content area -->
</section>
```

## Why This Approach Works

1. **Single Source of Truth**: All layout logic lives in one place
2. **Maintainable**: Changes to breakpoints only require updating the parent grid
3. **Predictable**: Components always align perfectly with the global grid
4. **Flexible**: Easy to create full-width sections, standard content, or off-grid elements
5. **Browser Support**: CSS Grid and named lines have excellent modern browser support

## Conclusion

By combining CSS Grid's named lines with subgrid, we've created a layout system that's both powerful and maintainable. Components become layout-agnostic while still participating in a cohesive, responsive design system.

This approach eliminates the complexity of managing breakpoints across multiple components while ensuring perfect alignment and consistent spacing throughout your application. It's a paradigm shift from component-based responsive design to system-based responsive design.

The result? Cleaner code, easier maintenance, and a more consistent user experience across all devices.
