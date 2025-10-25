# Category Hero Images Directory

Place your featured category hero images here. These are the large showcase images on the homepage.

## Required Files

Add exactly these three files (names must match exactly):

1. `portraits-hero.jpg` - Your best portrait photography
2. `landscapes-hero.jpg` - Your best landscape photography  
3. `wildlife-hero.jpg` - Your best wildlife photography

## Specifications

- **Format**: High-quality JPG
- **Size**: 1800x1200px or larger (landscape orientation)
- **Aspect ratio**: 3:2 or 4:3 recommended
- **File size**: Aim for under 800KB each (optimize for web)

## Customization

If you want to change the category names or add more categories, edit the `featured_categories` array in `src/app.rs` in the `HomePage` component (around line 95).

## How It Works

The server automatically looks for these three files and displays them on the homepage. If a file is missing, it will fall back to looking in the parent `/public/images/` directory.
