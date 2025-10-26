# Content Directory

This directory contains the content for your About page.

## About Page Content

### Profile Image
Place your profile image in this directory with the name `profile` and one of these extensions:
- `profile.jpg`
- `profile.jpeg`
- `profile.png`
- `profile.webp`

The system will automatically find and use the first matching file.

**Fallback:** If no profile image is found here, it will look for `/images/profile.jpg`

### About Text
Create a file named `about.txt` in this directory with your about text.

**Format:**
- Separate paragraphs with blank lines (double newline)
- Each paragraph will be rendered as a separate `<p>` tag

**Example `about.txt`:**
```
Hello! I'm a passionate photographer specializing in capturing the beauty of everyday moments.
With over 10 years of experience, I've worked on various projects ranging from landscapes to portraits.

My photography style focuses on natural lighting and authentic emotions.
I believe every photograph tells a unique story, and I'm here to help you tell yours.

Based in New York, I'm available for commissions and collaborations.
```

**Fallback:** If no `about.txt` file is found, default placeholder text will be displayed.

## Environment Variable Override

You can override the content directory location by setting the `ABOUT_CONTENT_PATH` environment variable:
```
ABOUT_CONTENT_PATH=/custom/path/to/content
```
