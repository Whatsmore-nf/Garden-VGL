#ifndef VGL_CANVAS_HPP
#define VGL_CANVAS_HPP

#include "vgl.hpp"
#include <vector>
#include <string>
#include <cstdint>

namespace vgl {

class Canvas {
public:
    uint32_t width = 0;
    uint32_t height = 0;
    std::vector<float> pixels; // RGBA float values [0, 255]
    
    Canvas() = default;
    Canvas(uint32_t w, uint32_t h);
    
    void clear(uint8_t r, uint8_t g, uint8_t b, uint8_t a = 255);
    void set_pixel(int x, int y, uint8_t r, uint8_t g, uint8_t b, uint8_t a = 255);
    std::tuple<uint8_t, uint8_t, uint8_t, uint8_t> get_pixel(int x, int y) const;
    
    bool save_png(const std::string& filename) const;
};

// Draw a line using Bresenham's algorithm with Wu antialiasing for thin lines
void draw_line(Canvas& canvas, int x0, int y0, int x1, int y1,
               uint8_t r, uint8_t g, uint8_t b, uint8_t a = 255, int width = 1);

// Draw a circle using midpoint algorithm
void draw_circle(Canvas& canvas, int cx, int cy, int radius,
                 uint8_t r, uint8_t g, uint8_t b, uint8_t a = 255, int stroke_width = 1);

// Fill a rectangle
void fill_rect(Canvas& canvas, int x, int y, int w, int h,
               uint8_t r, uint8_t g, uint8_t b, uint8_t a = 255);

// Fill a circle
void fill_circle(Canvas& canvas, int cx, int cy, int radius,
                 uint8_t r, uint8_t g, uint8_t b, uint8_t a = 255);

} // namespace vgl

#endif // VGL_CANVAS_HPP
