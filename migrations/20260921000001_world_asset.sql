CREATE TABLE game_assets (
    asset_key TEXT PRIMARY KEY,
    content_type TEXT NOT NULL,
    data TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO game_assets (asset_key, content_type, data)
VALUES (
    'world.json',
    'application/json',
    $asset$
{
  "background": [17, 29, 39, 255],
  "ground": [45, 94, 65, 255],
  "dot": [65, 126, 76, 180],
  "dots": [[48, 48], [112, 48], [176, 48], [240, 48], [304, 48], [368, 48], [432, 48], [496, 48], [560, 48], [624, 48], [688, 48], [752, 48], [48, 112], [112, 112], [176, 112], [240, 112], [304, 112], [368, 112], [432, 112], [496, 112], [560, 112], [624, 112], [688, 112], [752, 112]],
  "decorations": [
    { "kind": "house", "x": 170, "y": 180, "w": 120, "h": 90, "color": [115, 78, 49, 255] },
    { "kind": "door", "x": 195, "y": 205, "w": 70, "h": 65, "color": [76, 49, 36, 255] },
    { "kind": "shelter", "x": 550, "y": 480, "w": 140, "h": 80, "color": [101, 72, 51, 255] },
    { "kind": "water", "x": 625, "y": 190, "w": 34, "h": 34, "color": [33, 110, 136, 255] },
    { "kind": "water_inner", "x": 625, "y": 190, "w": 25, "h": 25, "color": [47, 143, 165, 255] }
  ]
}
    $asset$
)
ON CONFLICT (asset_key) DO UPDATE
SET content_type = EXCLUDED.content_type,
    data = EXCLUDED.data,
    updated_at = now();
