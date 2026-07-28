#include "canvas.hpp"
#include <png.h>
#include <cstring>
#include <cmath>
#include <algorithm>

namespace vgl {

Canvas::Canvas(uint32_t w, uint32_t h) : width(w), height(h) {
    pixels.resize(w * h * 4, 0.0f);
}

void Canvas::clear(uint8_t r, uint8_t g, uint8_t b, uint8_t a) {
    for (uint32_t y = 0; y < height; ++y) {
        for (uint32_t x = 0; x < width; ++x) {
            size_t idx = (y * width + x) * 4;
            pixels[idx] = static_cast<float>(r);
            pixels[idx + 1] = static_cast<float>(g);
            pixels[idx + 2] = static_cast<float>(b);
            pixels[idx + 3] = static_cast<float>(a);
        }
    }
}

void Canvas::set_pixel(int x, int y, uint8_t r, uint8_t g, uint8_t b, uint8_t a) {
    if (x < 0 || x >= static_cast<int>(width) || y < 0 || y >= static_cast<int>(height)) {
        return;
    }
    size_t idx = (y * width + x) * 4;
    
    // Alpha blending with premultiplied alpha
    float src_a = a / 255.0f;
    float dst_a = pixels[idx + 3] / 255.0f;
    float out_a = src_a + dst_a * (1.0f - src_a);
    
    if (out_a > 0.0f) {
        pixels[idx] = (r * src_a + pixels[idx] * dst_a * (1.0f - src_a)) / out_a;
        pixels[idx + 1] = (g * src_a + pixels[idx + 1] * dst_a * (1.0f - src_a)) / out_a;
        pixels[idx + 2] = (b * src_a + pixels[idx + 2] * dst_a * (1.0f - src_a)) / out_a;
        pixels[idx + 3] = out_a * 255.0f;
    }
}

std::tuple<uint8_t, uint8_t, uint8_t, uint8_t> Canvas::get_pixel(int x, int y) const {
    if (x < 0 || x >= static_cast<int>(width) || y < 0 || y >= static_cast<int>(height)) {
        return {0, 0, 0, 0};
    }
    size_t idx = (y * width + x) * 4;
    uint8_t r = static_cast<uint8_t>(std::clamp(pixels[idx], 0.0f, 255.0f));
    uint8_t g = static_cast<uint8_t>(std::clamp(pixels[idx + 1], 0.0f, 255.0f));
    uint8_t b = static_cast<uint8_t>(std::clamp(pixels[idx + 2], 0.0f, 255.0f));
    uint8_t a = static_cast<uint8_t>(std::clamp(pixels[idx + 3], 0.0f, 255.0f));
    return {r, g, b, a};
}

bool Canvas::save_png(const std::string& filename) const {
    FILE* fp = fopen(filename.c_str(), "wb");
    if (!fp) return false;
    
    png_structp png_ptr = png_create_write_struct(PNG_LIBPNG_VER_STRING, nullptr, nullptr, nullptr);
    if (!png_ptr) { fclose(fp); return false; }
    
    png_infop info_ptr = png_create_info_struct(png_ptr);
    if (!info_ptr) {
        png_destroy_write_struct(&png_ptr, nullptr);
        fclose(fp);
        return false;
    }
    
    if (setjmp(png_jmpbuf(png_ptr))) {
        png_destroy_write_struct(&png_ptr, &info_ptr);
        fclose(fp);
        return false;
    }
    
    png_init_io(png_ptr, fp);
    png_set_IHDR(png_ptr, info_ptr, width, height, 8, PNG_COLOR_TYPE_RGBA,
                 PNG_INTERLACE_NONE, PNG_COMPRESSION_TYPE_DEFAULT, PNG_FILTER_TYPE_DEFAULT);
    png_write_info(png_ptr, info_ptr);
    
    // Convert float pixels to u8
    std::vector<png_byte> row_data(width * 4);
    for (uint32_t y = 0; y < height; ++y) {
        for (uint32_t x = 0; x < width; ++x) {
            size_t idx = (y * width + x) * 4;
            row_data[x * 4 + 0] = static_cast<png_byte>(std::clamp(pixels[idx], 0.0f, 255.0f));
            row_data[x * 4 + 1] = static_cast<png_byte>(std::clamp(pixels[idx + 1], 0.0f, 255.0f));
            row_data[x * 4 + 2] = static_cast<png_byte>(std::clamp(pixels[idx + 2], 0.0f, 255.0f));
            row_data[x * 4 + 3] = static_cast<png_byte>(std::clamp(pixels[idx + 3], 0.0f, 255.0f));
        }
        png_write_row(png_ptr, row_data.data());
    }
    
    png_write_end(png_ptr, nullptr);
    png_destroy_write_struct(&png_ptr, &info_ptr);
    fclose(fp);
    return true;
}

// Simple line drawing (can be enhanced with Wu antialiasing)
void draw_line(Canvas& canvas, int x0, int y0, int x1, int y1,
               uint8_t r, uint8_t g, uint8_t b, uint8_t a, int width) {
    int dx = std::abs(x1 - x0);
    int dy = std::abs(y1 - y0);
    int sx = (x0 < x1) ? 1 : -1;
    int sy = (y0 < y1) ? 1 : -1;
    int err = (dx > dy ? dx : -dy) / 2;
    
    while (true) {
        if (width <= 1) {
            canvas.set_pixel(x0, y0, r, g, b, a);
        } else {
            // Draw a small circle at each point for thick lines
            fill_circle(canvas, x0, y0, width / 2, r, g, b, a);
        }
        
        if (x0 == x1 && y0 == y1) break;
        int e2 = err;
        if (e2 > -dx) { err -= dy; x0 += sx; }
        if (e2 < dy) { err += dx; y0 += sy; }
    }
}

void draw_circle(Canvas& canvas, int cx, int cy, int radius,
                 uint8_t r, uint8_t g, uint8_t b, uint8_t a, int stroke_width) {
    int x = radius;
    int y = 0;
    int err = 0;
    
    while (x >= y) {
        if (stroke_width <= 1) {
            canvas.set_pixel(cx + x, cy + y, r, g, b, a);
            canvas.set_pixel(cx + y, cy + x, r, g, b, a);
            canvas.set_pixel(cx - y, cy + x, r, g, b, a);
            canvas.set_pixel(cx - x, cy + y, r, g, b, a);
            canvas.set_pixel(cx - x, cy - y, r, g, b, a);
            canvas.set_pixel(cx - y, cy - x, r, g, b, a);
            canvas.set_pixel(cx + y, cy - x, r, g, b, a);
            canvas.set_pixel(cx + x, cy - y, r, g, b, a);
        } else {
            // Draw filled circles at each point for thick stroke
            int sw = stroke_width / 2;
            fill_circle(canvas, cx + x, cy + y, sw, r, g, b, a);
            fill_circle(canvas, cx + y, cy + x, sw, r, g, b, a);
            fill_circle(canvas, cx - y, cy + x, sw, r, g, b, a);
            fill_circle(canvas, cx - x, cy + y, sw, r, g, b, a);
            fill_circle(canvas, cx - x, cy - y, sw, r, g, b, a);
            fill_circle(canvas, cx - y, cy - x, sw, r, g, b, a);
            fill_circle(canvas, cx + y, cy - x, sw, r, g, b, a);
            fill_circle(canvas, cx + x, cy - y, sw, r, g, b, a);
        }
        
        y++;
        if (err <= 0) {
            err += 2 * y + 1;
        }
        if (err > 0) {
            x--;
            err -= 2 * x + 1;
        }
    }
}

void fill_rect(Canvas& canvas, int x, int y, int w, int h,
               uint8_t r, uint8_t g, uint8_t b, uint8_t a) {
    for (int yy = y; yy < y + h; ++yy) {
        for (int xx = x; xx < x + w; ++xx) {
            canvas.set_pixel(xx, yy, r, g, b, a);
        }
    }
}

void fill_circle(Canvas& canvas, int cx, int cy, int radius,
                 uint8_t r, uint8_t g, uint8_t b, uint8_t a) {
    for (int y = -radius; y <= radius; ++y) {
        for (int x = -radius; x <= radius; ++x) {
            if (x * x + y * y <= radius * radius) {
                canvas.set_pixel(cx + x, cy + y, r, g, b, a);
            }
        }
    }
}

} // namespace vgl
