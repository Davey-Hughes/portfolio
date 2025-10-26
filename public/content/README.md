# Content Directory

This directory contains all the dynamic content for your photography portfolio site.

## Site Configuration (`config.txt`)

The `config.txt` file contains all site-wide settings including contact information.

**Format:**
- One setting per line in `key = value` format
- Lines starting with `#` are comments and ignored
- Empty lines are ignored

**Required Fields:**
- `site_name` - Your name or business name
- `site_tagline` - Appears below your name on the home page
- `site_copyright` - Footer copyright text

**Contact Information:**
You can add ANY custom contact fields you want! They will automatically appear on the contact page.
The key names are automatically formatted as labels (e.g., `email` → "Email", `phone_number` → "Phone Number").

**Example `config.txt`:**
```
# Core site settings
site_name = Your Name
site_tagline = Photography
site_copyright = © 2025 Your Photography. All rights reserved.

# Contact information - add as many fields as you want!
email = contact@yoursite.com
phone = +1 (555) 123-4567
location = New York, NY
website = www.yoursite.com
instagram = @yourhandle
linkedin = linkedin.com/in/yourprofile
studio_address = 123 Main St, New York, NY 10001
```

## About Page Content

### Profile Image
Place your profile image in this directory with the name `profile` and one of these extensions:
- `profile.jpg`
- `profile.jpeg`
- `profile.png`
- `profile.webp`

The system will automatically find and use the first matching file.

**Fallback:** If no profile image is found here, it will look for `/images/profile.jpg`

### About Text (`about.txt`)
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

## Environment Variable Overrides

You can override the content directory location by setting environment variables:
```bash
ABOUT_CONTENT_PATH=/custom/path/to/content
CONFIG_PATH=/custom/path/to/config.txt
```
