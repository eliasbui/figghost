// The sky behind the Weather tab, drawn for every pixel: a gradient lit by the sun or the moon, stars, clouds that
// shade themselves against the light, rain and snow in three depths, fog, haze, frost, wind, leaves and lightning.
// The app says what the sky is like in `U`; the leaves come from the app as well, already moved.

#include <metal_stdlib>
using namespace metal;

struct U {
    float4 a; // width and height in pixels, time in seconds, 1 by day
    float4 b; // progress of the sun or the moon, cloud, rain, thunder
    float4 c; // snow, fog, cold, wind
    float4 d; // direction of the wind (-1 or 1), haze, flash of lightning, where the bolt falls (0 to 1)
    float4 e; // colour of the leaves, how many there are
    float4 f; // pixels to a point
};

struct VOut {
    float4 pos [[position]];
    float2 uv;
};

vertex VOut sky_vs(uint vid [[vertex_id]]) {
    float2 p = float2(vid == 1 ? 3.0 : -1.0, vid == 2 ? 3.0 : -1.0);
    VOut o;
    o.pos = float4(p, 0.0, 1.0);
    o.uv = float2(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return o;
}

static float h11(float n) { return fract(sin(n * 127.1 + 311.7) * 43758.5453); }

static float h21(float2 p) {
    float3 q = fract(float3(p.xyx) * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

static float vnoise(float2 p) {
    float2 i = floor(p), f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(h21(i), h21(i + float2(1, 0)), f.x), mix(h21(i + float2(0, 1)), h21(i + float2(1, 1)), f.x), f.y);
}

static float fbm4(float2 p) {
    float v = 0.0, a = 0.5;
    for (int i = 0; i < 4; i++) {
        v += a * vnoise(p);
        p = p * 2.03 + float2(17.1, 9.2);
        a *= 0.5;
    }
    return v;
}

// How much cloud there is at a point, in one of three layers that drift at their own speed.
static float cloud_density(float2 p, float t, float wind, float dir, float cover, float k) {
    float speed = (0.006 + 0.05 * wind) * (1.0 + 0.55 * k);
    // Clouds are wider than they are tall, and the near ones are bigger.
    float2 q = float2(p.x, p.y * 1.8) * (0.95 + 0.75 * k) + float2(t * speed * dir + k * 7.0, k * 3.1);
    float warp = fbm4(q * 0.6 + t * 0.01);
    float n = fbm4(q + warp * 0.45);
    float thr = 0.76 - 0.46 * cover;
    // A thin cover has clear edges; a full one melts into a flat grey.
    float soft = mix(0.14, 0.55, smoothstep(0.6, 1.0, cover));
    return smoothstep(thr, thr + soft, n) * smoothstep(0.0, 0.10, cover);
}

// Rain of one depth: thin streaks that fall in columns, the near ones long and bright, the far ones short and soft.
static float rain_layer(float2 uv, float aspect, float t, float k, float rain, float slant) {
    float cols = k < 0.5 ? 80.0 : (k < 1.5 ? 50.0 : 30.0);
    float2 p = float2(uv.x * aspect + uv.y * slant, uv.y);
    float2 g = float2(p.x * cols, p.y * cols * 0.28);
    float col = floor(g.x);
    float speed = (3.0 + 5.5 * rain) * (0.8 + 0.4 * h11(col + k * 31.0)) * (1.0 + 0.3 * k);
    g.y -= t * speed;
    float2 id = floor(g);
    float2 f = fract(g);
    float hh = h21(id + k * 17.3);
    if (hh > 0.04 + 0.22 * rain) return 0.0;
    float cx = 0.5 + (h21(id + 7.7) - 0.5) * 0.5;
    float width = (k < 0.5 ? 0.14 : (k < 1.5 ? 0.09 : 0.055)) * (1.2 - 0.4 * rain);
    float line = smoothstep(width, 0.0, abs(f.x - cx));
    float reach = 0.35 + 0.5 * h21(id + 2.4);
    float along = smoothstep(0.0, reach, f.y) * (1.0 - smoothstep(reach, reach + 0.12, f.y));
    return line * along * (k < 0.5 ? 0.30 : (k < 1.5 ? 0.45 : 0.65)) * (0.6 + 0.4 * h21(id + 5.5));
}

// Snow of one depth: round flakes that sway as they fall; near flakes are big and out of focus.
static float snow_layer(float2 uv, float aspect, float t, float k, float amount, float wind, float dir) {
    float scale = 9.0 + k * 8.0;
    float2 p = float2(uv.x * aspect, uv.y) * scale;
    p.y -= t * (0.55 + 0.3 * k);
    p.x += t * wind * dir * (0.6 + 0.4 * k);
    float2 id = floor(p);
    float2 f = fract(p);
    float hh = h21(id + k * 9.1);
    if (hh > 0.08 + 0.4 * amount) return 0.0;
    float2 c = 0.5 + (float2(h21(id + 3.3), h21(id + 5.9)) - 0.5) * 0.35;
    // Each flake sways inside its own cell as it comes down, and spins out of the way of its neighbours.
    c.x += sin(t * (0.9 + hh * 3.0) + hh * 40.0) * 0.28;
    float d = length(f - c);
    float r = (0.07 + 0.08 * h21(id + 1.1)) * (1.0 + 0.5 * k);
    float soft = 0.02 + 0.07 * k;
    return smoothstep(r + soft, r - soft * 0.4, d) * (0.9 - 0.18 * k);
}

// Streaks of wind: long thin lines that wave a little and run in the direction of the wind.
static float wind_layer(float2 uv, float aspect, float t, float k, float wind, float dir) {
    float rows = 16.0 + 9.0 * k;
    float2 g = float2(uv.x * aspect * 2.0 - t * dir * (0.8 + 3.0 * wind) * (1.0 + 0.4 * k), uv.y * rows);
    float row = floor(g.y);
    g.x += h11(row + k * 13.0) * 9.0;
    float2 id = float2(floor(g.x), row);
    float2 f = float2(fract(g.x), fract(g.y));
    float hh = h21(id + k * 5.0);
    if (hh > 0.16 * wind) return 0.0;
    float wave = 0.12 * sin(g.x * 5.0 + t * 2.5 + hh * 40.0);
    float thin = smoothstep(0.09, 0.0, abs(f.y - 0.5 - wave)) * 0.7;
    float along = smoothstep(0.0, 0.5, f.x) * (1.0 - smoothstep(0.5, 1.0, f.x));
    return thin * along;
}

fragment float4 sky_fs(VOut in [[stage_in]], constant U& u [[buffer(0)]], constant float4* leaves [[buffer(1)]]) {
    float2 res = u.a.xy;
    float t = u.a.z;
    float day = u.a.w;
    float prog = u.b.x, cloud = u.b.y, rain = u.b.z, thunder = u.b.w;
    float snow = u.c.x, fog = u.c.y, cold = u.c.z, wind = u.c.w;
    float dir = u.d.x, haze = u.d.y, flash = u.d.z, boltx = u.d.w;
    float3 leaf_colour = u.e.xyz;
    int leaf_count = int(u.e.w);
    float aspect = res.x / res.y;
    float2 uv = in.uv;
    float2 p = float2(uv.x * aspect, uv.y);

    float el = sin(3.14159265 * clamp(prog, 0.0, 1.0));
    float low = 1.0 - smoothstep(0.0, 0.38, el);
    float2 sun = float2(0.10 + 0.80 * prog, 0.90 - 0.64 * el);
    float2 sp = float2(sun.x * aspect, sun.y);
    float storm = clamp(max(rain, thunder * 0.9), 0.0, 1.0);
    float grey = clamp(smoothstep(0.35, 1.0, cloud) * 0.85 + rain * 0.5 + fog * 0.4 + snow * 0.4, 0.0, 1.0);

    // The colour of the air.
    float3 zenith, horizon;
    if (day > 0.5) {
        zenith = mix(float3(0.09, 0.30, 0.70), float3(0.14, 0.18, 0.42), low);
        horizon = mix(float3(0.60, 0.80, 0.97), float3(1.0, 0.55, 0.32), low);
    } else {
        zenith = float3(0.010, 0.020, 0.075);
        horizon = float3(0.055, 0.085, 0.19);
    }
    float3 col = mix(zenith, horizon, pow(uv.y, 0.85));
    float lum = dot(col, float3(0.30, 0.59, 0.11));
    float3 dull = float3(lum) * float3(0.92, 0.96, 1.02) * (day > 0.5 ? 1.25 : 1.1);
    col = mix(col, dull * (1.0 - 0.5 * storm), grey);

    // Stars come out where the night is clear, and a faint band of the Milky Way crosses them.
    if (day < 0.5) {
        float2 g = p * 64.0;
        float2 id = floor(g);
        float2 f = fract(g);
        float hh = h21(id);
        if (hh > 0.962) {
            float2 c = 0.5 + (float2(h21(id + 1.7), h21(id + 4.1)) - 0.5) * 0.6;
            float tw = 0.55 + 0.45 * sin(t * (1.0 + 3.0 * hh) + hh * 60.0);
            float s = smoothstep(0.17, 0.0, length(f - c)) * tw;
            col += float3(0.85, 0.9, 1.0) * s * (1.0 - grey) * smoothstep(1.0, 0.3, uv.y) * 1.2;
        }
        float band = smoothstep(0.35, 0.0, abs(uv.y - (0.55 - uv.x * 0.35)));
        col += float3(0.05, 0.06, 0.12) * fbm4(p * 5.0) * band * (1.0 - grey);
    }

    float2 to_light = sp - p;
    float dl = length(to_light);
    float3 sun_colour = mix(float3(1.0, 0.97, 0.86), float3(1.0, 0.60, 0.28), low);
    sun_colour = mix(sun_colour, float3(1.0, 0.42, 0.18), haze * 0.6);
    float clear = (1.0 - grey * 0.55) * (1.0 - haze * 0.35) * (1.0 - max(storm, snow) * 0.9) * (1.0 - smoothstep(0.5, 1.0, cloud) * 0.85);
    if (day > 0.5) {
        float bloom = exp(-dl * 3.2) * 0.40 + exp(-dl * 11.0) * 0.55 + exp(-dl * 34.0) * 0.7;
        col += sun_colour * bloom * clear;
        // Rays fan out from the sun and are cut off by whatever stands in the way.
        float2 dv = p - sp;
        float ang = atan2(dv.y, dv.x);
        float rays = pow(vnoise(float2(ang * 6.0 + t * 0.04, 3.0)), 2.2) * exp(-dl * 2.3);
        col += sun_colour * rays * 0.16 * (1.0 - grey) * (1.0 - haze * 0.5);
        float disc = smoothstep(0.040, 0.035, dl);
        col = mix(col, mix(sun_colour, float3(1.0), 0.55) * 1.5, disc * clear);
    } else {
        float glow = exp(-dl * 6.0) * 0.30 + exp(-dl * 30.0) * 0.4;
        col += float3(0.62, 0.72, 1.0) * glow * (1.0 - grey * 0.7);
        if (dl < 0.040) {
            float2 local = (p - sp) / 0.040;
            float crater = fbm4(local * 2.4 + 5.0);
            float lit = smoothstep(-0.3, 0.7, dot(local, float2(-0.55, -0.35)) * 0.5 + 0.55);
            float3 moon = float3(0.95, 0.96, 1.0) * (0.72 + 0.28 * crater) * mix(0.50, 1.0, lit);
            col = mix(col, moon, smoothstep(1.0, 0.92, length(local)) * (1.0 - grey * 0.75));
        }
    }

    // Clouds, far to near. Each one is lit from the side of the sun, so the side away from it is in shade.
    float3 lit_cloud = day > 0.5 ? mix(float3(1.0), sun_colour, 0.35 + 0.3 * low) : float3(0.40, 0.46, 0.62);
    lit_cloud = mix(lit_cloud, float3(0.50, 0.54, 0.64), storm * (day > 0.5 ? 1.0 : 0.5));
    float3 shade_cloud = day > 0.5 ? mix(float3(0.56, 0.63, 0.76), float3(0.11, 0.13, 0.19), storm) : float3(0.05, 0.07, 0.13);
    float2 toward = dl > 0.001 ? to_light / dl : float2(0.0, -1.0);
    float cover = max(cloud, storm * 0.9);
    for (int k = 0; k < 3; k++) {
        float height = mix(1.0, 0.55, smoothstep(0.5, 1.0, uv.y));
        float d0 = cloud_density(p, t, wind, dir, cover, float(k)) * height;
        float d1 = cloud_density(p + toward * 0.035, t, wind, dir, cover, float(k)) * height;
        float d2 = cloud_density(p + toward * 0.09, t, wind, dir, cover, float(k)) * height;
        float below = cloud_density(p + float2(0.0, 0.04), t, wind, dir, cover, float(k)) * height;
        // Lit where less cloud lies toward the light, dark where more does, and dark at the flat base of a cloud.
        float lit = clamp(0.78 - (d1 - d0) * 2.6 - (d2 - d0) * 1.3 + (below - d0) * 1.1 - d0 * 0.20, 0.06, 1.0);
        float3 cc = mix(shade_cloud, lit_cloud, lit);
        cc += float3(0.70, 0.75, 1.0) * flash * (0.35 + 0.65 * d0);
        col = mix(col, cc, d0 * (0.92 - 0.14 * float(k)));
    }

    // Fog lies low and drifts.
    if (fog > 0.01) {
        float n = fbm4(float2(p.x * 1.3 + t * 0.02 * dir, p.y * 2.6 + t * 0.01));
        float3 fog_colour = (day > 0.5 ? float3(0.74, 0.77, 0.82) : float3(0.14, 0.16, 0.24)) * (1.0 + 0.5 * flash);
        col = mix(col, fog_colour, clamp(fog * (0.30 + 0.9 * n) * (0.55 + 0.45 * uv.y), 0.0, 0.92));
    }

    // Dirty air: a brown veil that is thickest at the horizon, and dust drifting in it.
    if (haze > 0.01) {
        float3 veil = (day > 0.5 ? float3(0.72, 0.54, 0.32) : float3(0.18, 0.13, 0.09));
        col = mix(col, veil, haze * 0.88 * (0.55 + 0.45 * uv.y));
        float dust = 0.0;
        for (int k = 0; k < 2; k++) {
            float2 g = p * (30.0 + 22.0 * float(k)) + float2(t * (0.5 + 0.3 * float(k)) * dir, sin(t * 0.3 + float(k)) * 2.0);
            float2 id = floor(g);
            float2 f = fract(g);
            if (h21(id + float(k) * 7.0) < 0.06 + 0.16 * haze) {
                float2 c = 0.5 + (float2(h21(id + 2.2), h21(id + 8.8)) - 0.5) * 0.6;
                dust += smoothstep(0.10, 0.0, length(f - c)) * 0.5;
            }
        }
        col += float3(0.78, 0.62, 0.40) * dust * haze * 0.22;
    }

    // Wind.
    if (wind > 0.1) {
        float s = 0.0;
        for (int k = 0; k < 2; k++) s += wind_layer(uv, aspect, t, float(k), wind, dir);
        col += float3(0.92, 0.96, 1.0) * s * 0.10 * wind;
    }

    // Rain in three depths, slanted by the wind, and the mist it throws up.
    if (rain > 0.02) {
        float slant = dir * (0.10 + 0.55 * wind);
        float r = 0.0;
        for (int k = 0; k < 3; k++) r += rain_layer(uv, aspect, t, float(k), rain, slant);
        float3 drop = mix(float3(0.62, 0.75, 0.95), float3(0.9, 0.94, 1.0), flash);
        col += drop * r * (0.50 + 0.30 * rain) + float3(0.8, 0.85, 0.95) * flash * r * 0.5;
        float mist = fbm4(float2(p.x * 2.5 - t * 0.15 * dir, p.y * 4.0 - t * 0.3));
        col = mix(col, float3(0.50, 0.55, 0.62), rain * 0.18 * mist * smoothstep(0.35, 1.0, uv.y));
    }

    // Snow in three depths.
    if (snow > 0.02) {
        float s = 0.0;
        for (int k = 0; k < 3; k++) s += snow_layer(uv, aspect, t, float(k), snow, wind, dir);
        col = mix(col, float3(0.97, 0.98, 1.0), clamp(s, 0.0, 1.0) * 0.92);
    }

    // Leaves, each tumbling in three dimensions: a leaf turned edge-on is thin.
    for (int i = 0; i < leaf_count; i++) {
        float4 L = leaves[i];
        float2 c = float2(L.x * aspect, L.y);
        float size = L.w;
        float2 d = p - c;
        if (dot(d, d) > size * size * 3.2) continue;
        float ca = cos(L.z), sa = sin(L.z);
        float2 q = float2(d.x * ca + d.y * sa, -d.x * sa + d.y * ca);
        float flip = 0.30 + 0.70 * abs(cos(L.z * 0.6 + float(i) * 1.7));
        float lx = q.x / size;
        float ly = q.y / (size * 0.52 * flip);
        float body = (1.0 - lx * lx) * (0.60 + 0.40 * (0.5 - 0.5 * lx));
        float inside = smoothstep(0.0, 0.14, body - abs(ly)) * step(abs(lx), 1.0);
        if (inside > 0.0) {
            float vein = smoothstep(0.07, 0.0, abs(ly)) * 0.35;
            float side = 0.74 + 0.26 * (1.0 - abs(ly)) + 0.12 * sign(ly) * ca;
            float3 lc = leaf_colour * side * (0.55 + 0.45 * flip) * (1.0 - vein);
            col = mix(col, lc, inside * 0.96);
        }
    }

    // Cold: frost grows in from the edges, the air goes blue, ice glints and breath hangs low.
    if (cold > 0.01) {
        float2 e = min(uv, 1.0 - uv) * float2(aspect, 1.0);
        float edge = min(e.x, e.y);
        // The edge of the frost is ragged and finely grained, like breath on glass.
        float grain = fbm4(p * 9.0) * 0.55 + fbm4(p * 34.0) * 0.30;
        float mask = smoothstep(0.03 + 0.12 * cold + grain * 0.12, 0.0, edge);
        col = mix(col, float3(0.86, 0.95, 1.0), mask * cold * 0.62);
        col = mix(col, col * float3(0.82, 0.94, 1.16), cold * 0.45);
        float2 g = p * 38.0;
        float2 id = floor(g);
        float2 f = fract(g);
        if (h21(id + 3.0) > 0.975) {
            float tw = pow(max(0.0, sin(t * 1.6 + h21(id) * 30.0)), 3.0);
            float2 c = abs(f - 0.5);
            float star = smoothstep(0.30, 0.0, min(c.x, c.y) * 4.0 + max(c.x, c.y) * 0.6);
            col += float3(0.85, 0.96, 1.0) * star * tw * cold * 0.9;
        }
        float breath = fbm4(float2(p.x * 3.0 + t * 0.05, p.y * 3.0 - t * 0.12));
        col += float3(0.85, 0.92, 1.0) * breath * cold * 0.10 * smoothstep(0.4, 1.0, uv.y);
    }

    // Lightning: the whole sky lights up, and a bolt with a branch comes down.
    col += float3(0.55, 0.60, 0.95) * flash * 0.32;
    if (flash > 0.15) {
        // The bolt is a chain of straight pieces, each leaning from where the last ended, and one branch.
        float ys = clamp(uv.y, 0.0, 0.999) * 16.0;
        int seg = int(floor(ys));
        float fr = ys - floor(ys);
        float seed = boltx * 97.0;
        float acc = 0.0, at_branch = 0.0;
        for (int i = 0; i < 16; i++) {
            if (i < seg) acc += (h11(float(i) + seed) - 0.5) * 0.11;
            if (i < 5) at_branch = acc;
        }
        float xx = boltx + acc + (h11(float(seg) + seed) - 0.5) * 0.11 * fr;
        float fade = smoothstep(0.90, 0.60, uv.y);
        float dx = abs(uv.x - xx) * aspect;
        float width = 0.0035 + 0.002 * (1.0 - uv.y);
        float core = smoothstep(width, 0.0, dx) * fade;
        float glow = exp(-dx * 42.0) * 0.50 * fade;
        float ys2 = max(uv.y - 0.31, 0.0) * 16.0;
        float bd = (boltx < 0.5 ? 1.0 : -1.0) * ys2 * 0.040 + (vnoise(float2(ys2 * 1.7, seed)) - 0.5) * 0.05;
        float bx = boltx + at_branch + bd;
        float bdx = abs(uv.x - bx) * aspect;
        float branch = smoothstep(width * 0.7, 0.0, bdx) * smoothstep(0.31, 0.34, uv.y) * smoothstep(0.78, 0.50, uv.y) * 0.85;
        col += float3(0.85, 0.9, 1.0) * (core * 2.4 + glow * 1.4 + branch * 1.3) * smoothstep(0.15, 0.7, flash);
    }

    // Quiet the brightest sky behind the text, and cut the corners of the page.
    col *= 0.80;
    col = col / (1.0 + col * 0.12);
    float2 pts = res / u.f.x;
    float2 hp = pts * 0.5;
    float2 q = abs(in.uv * pts - hp) - (hp - 18.0);
    float sd = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - 18.0;
    float mask = smoothstep(0.5, -0.5, sd);
    col = clamp(col, 0.0, 1.0);
    // A whisper of noise hides the bands that 8 bits leave in a smooth sky.
    col += (h21(in.pos.xy) - 0.5) / 255.0;
    return float4(col * mask, mask);
}
