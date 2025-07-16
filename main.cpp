// main.cpp

#include <iostream>
#include <fstream>
#include <string>
#include <map>
#include <vector>
#include <tuple>
#include <memory>
#include <algorithm>
#include <cstdlib>

#include <curl/curl.h>
#include <ncurses.h>
#include <vlc/vlc.h>
#include <nlohmann/json.hpp>

using json = nlohmann::json;

// ─────────────────────────────────────────────────────────────────────────────
// Data structures & Tree Node
// ─────────────────────────────────────────────────────────────────────────────

struct Track {
    std::string id, name, album, artist;
};

struct Node {
    std::string name;
    Track* track;                     // non-nullptr only for leaf (track) nodes
    Node* parent;
    std::vector<std::unique_ptr<Node>> children;
    bool expanded = false;
    int depth = 0;
    Node(std::string n, Track* t, Node* p)
      : name(std::move(n)), track(t), parent(p) {
        depth = p ? p->depth + 1 : 0;
    }
};

// ─────────────────────────────────────────────────────────────────────────────
// Sort children alphabetically by node->name
// ─────────────────────────────────────────────────────────────────────────────

void sort_tree(Node* node) {
    std::sort(node->children.begin(), node->children.end(),
              [](auto& a, auto& b){ return a->name < b->name; });
    for (auto& child : node->children) {
        sort_tree(child.get());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// libcurl callbacks & helpers
// ─────────────────────────────────────────────────────────────────────────────

static size_t write_cb(void* contents, size_t sz, size_t nmemb, void* up) {
    auto* s = static_cast<std::string*>(up);
    size_t total = sz * nmemb;
    s->append(static_cast<char*>(contents), total);
    return total;
}

json http_get_json(const std::string& url,
                   const std::map<std::string,std::string>& headers) {
    CURL* curl = curl_easy_init();
    if (!curl) throw std::runtime_error("curl_easy_init failed");
    std::string resp;
    struct curl_slist* hdrs = nullptr;
    for (auto& [k,v]: headers)
        hdrs = curl_slist_append(hdrs, (k + ": " + v).c_str());
    curl_easy_setopt(curl, CURLOPT_URL, url.c_str());
    curl_easy_setopt(curl, CURLOPT_HTTPHEADER, hdrs);
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, write_cb);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, &resp);
    curl_easy_setopt(curl, CURLOPT_SSL_VERIFYPEER, 0L);
    curl_easy_perform(curl);
    curl_slist_free_all(hdrs);
    curl_easy_cleanup(curl);
    return json::parse(resp);
}

json http_post_json(const std::string& url,
                    const json& payload,
                    const std::map<std::string,std::string>& headers) {
    CURL* curl = curl_easy_init();
    if (!curl) throw std::runtime_error("curl_easy_init failed");
    std::string resp, body = payload.dump();
    struct curl_slist* hdrs = curl_slist_append(nullptr, "Content-Type: application/json");
    for (auto& [k,v]: headers)
        hdrs = curl_slist_append(hdrs, (k + ": " + v).c_str());
    curl_easy_setopt(curl, CURLOPT_URL, url.c_str());
    curl_easy_setopt(curl, CURLOPT_HTTPHEADER, hdrs);
    curl_easy_setopt(curl, CURLOPT_POSTFIELDS, body.c_str());
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, write_cb);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, &resp);
    curl_easy_setopt(curl, CURLOPT_SSL_VERIFYPEER, 0L);
    curl_easy_perform(curl);
    curl_slist_free_all(hdrs);
    curl_easy_cleanup(curl);
    return json::parse(resp);
}

// ─────────────────────────────────────────────────────────────────────────────
// Configuration & Authentication
// ─────────────────────────────────────────────────────────────────────────────

json load_config(const std::string& path) {
    std::ifstream in(path);
    if (in) {
        json j; in >> j;
        return j;
    }
    // Prompt and save
    std::string url, user, pass;
    std::cout << "Jellyfin server URL: "; std::getline(std::cin, url);
    std::cout << "Username: ";          std::getline(std::cin, user);
    std::cout << "Password: ";          std::getline(std::cin, pass);
    json j = { {"server_url",url}, {"username",user}, {"password",pass} };
    std::ofstream out(path);
    out << j.dump(2) << "\n";
    return j;
}

std::tuple<std::string,std::string,std::string>
authenticate(const json& cfg) {
    std::string base = cfg.at("server_url").get<std::string>();
    if (!base.empty() && base.back() == '/') base.pop_back();
    std::vector<std::string> cand = { base, base + "/jellyfin" };

    json payload = {
        {"Username", cfg.at("username").get<std::string>()},
        {"Pw",       cfg.at("password").get<std::string>()}
    };
    auto hdrs = std::map<std::string,std::string>{
        {"X-Emby-Authorization",
         R"(MediaBrowser Client="TUI", Device="cli", DeviceId="caitunes", Version="1.0")"}
    };

    for (auto& c : cand) {
        try {
            auto r = http_post_json(c + "/Users/AuthenticateByName", payload, hdrs);
            std::string token = r.value("AccessToken","");
            auto user = r.value("User", json::object());
            std::string uid = user.value("Id","");
            if (!token.empty() && !uid.empty()) return {token, uid, c};
        } catch(...) { }
    }
    std::cerr << "Authentication failed. Check URL/credentials.\n";
    std::exit(1);
}

// ─────────────────────────────────────────────────────────────────────────────
// Fetch audio tracks
// ─────────────────────────────────────────────────────────────────────────────

std::vector<Track> fetch_tracks(const std::string& base,
                                const std::string& token,
                                const std::string& user_id) {
    std::vector<Track> out;
    int start = 0, limit = 10000;
    auto hdrs = std::map<std::string,std::string>{{"X-Emby-Token",token}};
    while (true) {
        auto r = http_get_json(
          base + "/Users/" + user_id + "/Items?IncludeItemTypes=Audio"
                 "&Recursive=true&SortBy=Album,SortName&SortOrder=Ascending"
                 "&StartIndex=" + std::to_string(start)
                 + "&Limit="    + std::to_string(limit),
          hdrs
        );
        auto items = r.value("Items", json::array());
        if (items.empty()) break;
        for (auto& it : items) {
            out.push_back({
                it.value("Id",""),
                it.value("Name","Unknown"),
                it.value("Album","Unknown"),
                !it.value("AlbumArtist","").empty()
                  ? it.value("AlbumArtist","")
                  : (!it.value("Artists", json::array()).empty()
                     ? it["Artists"][0].value("Name","Unknown")
                     : std::string("Unknown"))
            });
        }
        if ((int)items.size() < limit) break;
        start += limit;
    }
    return out;
}

// ─────────────────────────────────────────────────────────────────────────────
// Build hierarchical tree (Artist→Album→Track), collapsed by default
// ─────────────────────────────────────────────────────────────────────────────

std::unique_ptr<Node> build_tree(const std::vector<Track>& tracks) {
    auto root = std::make_unique<Node>("Music Library", nullptr, nullptr);
    std::map<std::string,Node*> artist_map;

    for (auto& t : tracks) {
        Node* artist_node;
        auto it = artist_map.find(t.artist);
        if (it == artist_map.end()) {
            artist_node = new Node(t.artist, nullptr, root.get());
            root->children.emplace_back(artist_node);
            artist_map[t.artist] = artist_node;
        } else {
            artist_node = it->second;
        }
        // find or create album node
        Node* album_node = nullptr;
        for (auto& c : artist_node->children) {
            if (c->name == t.album) { album_node = c.get(); break; }
        }
        if (!album_node) {
            album_node = new Node(t.album, nullptr, artist_node);
            artist_node->children.emplace_back(album_node);
        }
        // add track under album (always leaf)
        album_node->children.emplace_back(
            std::make_unique<Node>(t.name, const_cast<Track*>(&t), album_node)
        );
    }

    // sort all levels alphabetically
    sort_tree(root.get());
    return root;
}

// ─────────────────────────────────────────────────────────────────────────────
// Flatten expanded nodes to a visible list
// ─────────────────────────────────────────────────────────────────────────────

void flatten(Node* node, std::vector<Node*>& out) {
    for (auto& c : node->children) {
        out.push_back(c.get());
        if (c->expanded) {
            flatten(c.get(), out);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Draw only the visible window portion of the tree
// ─────────────────────────────────────────────────────────────────────────────

void draw_tree(const std::vector<Node*>& vis, size_t cursor, size_t win_top) {
    erase();  // partial redraw to reduce flicker
    int data_lines = LINES - 1;
    int y = 0;
    for (size_t i = win_top; i < vis.size() && y < data_lines; ++i) {
        Node* n = vis[i];
        std::string indent(n->depth * 2, ' ');
        if (i == cursor) attron(A_REVERSE);
        mvprintw(y, 0, "%s%s", indent.c_str(), n->name.c_str());
        if (i == cursor) attroff(A_REVERSE);
        y++;
    }
    mvprintw(LINES - 1, 0,
        "↑/↓ scroll  → expand/enter  ← collapse/back  Enter=play  q=quit");
    refresh();
}

// ─────────────────────────────────────────────────────────────────────────────
// UI loop: handle cursor, window offset, keybindings, and playback
// ─────────────────────────────────────────────────────────────────────────────

void ui_loop(Node* root, const std::string& base, const std::string& token) {
    initscr(); cbreak(); noecho(); keypad(stdscr, TRUE);

    std::vector<Node*> visible;
    flatten(root, visible);

    size_t cursor = 0, win_top = 0;
    int data_lines = LINES - 1;

    libvlc_instance_t* ml = libvlc_new(0, nullptr);
    libvlc_media_player_t* player = nullptr;

    draw_tree(visible, cursor, win_top);

    int ch;
    while ((ch = getch()) != 'q') {
        Node* cur = visible[cursor];
        switch (ch) {
            case KEY_UP:
                if (cursor > 0) cursor--;
                break;
            case KEY_DOWN:
                if (cursor + 1 < visible.size()) cursor++;
                break;
            case KEY_RIGHT:
                if (!cur->children.empty()) {
                    cur->expanded = true;
                }
                break;
            case KEY_LEFT:
                if (cur->expanded) {
                    cur->expanded = false;
                } else if (cur->parent) {
                    // jump back to parent
                    for (size_t i = 0; i < visible.size(); ++i) {
                        if (visible[i] == cur->parent) {
                            cursor = i;
                            break;
                        }
                    }
                }
                break;
            case '\n':
                if (cur->track) {
                    if (player) libvlc_media_player_stop(player);
                    std::string url = base
                        + "/Audio/" + cur->track->id
                        + "/universal?AudioCodec=mp3&Container=mp3&api_key="
                        + token;
                    auto m = libvlc_media_new_location(ml, url.c_str());
                    player = libvlc_media_player_new_from_media(m);
                    libvlc_media_release(m);
                    libvlc_media_player_play(player);
                }
                break;
            default:
                break;
        }

        // recalc visible & adjust window
        visible.clear();
        flatten(root, visible);
        if (cursor < win_top) {
            win_top = cursor;
        } else if (cursor >= win_top + data_lines) {
            win_top = cursor - data_lines + 1;
        }

        draw_tree(visible, cursor, win_top);
    }

    if (player) {
        libvlc_media_player_stop(player);
        libvlc_media_player_release(player);
    }
    libvlc_release(ml);
    endwin();
}

// ─────────────────────────────────────────────────────────────────────────────
// main()
// ─────────────────────────────────────────────────────────────────────────────

int main() {
    curl_global_init(CURL_GLOBAL_DEFAULT);

    std::string cfg_path = std::string(getenv("HOME")) + "/.caitunes_config.json";
    json cfg = load_config(cfg_path);

    auto [token,user_id,base] = authenticate(cfg);
    auto tracks = fetch_tracks(base, token, user_id);

    auto root = build_tree(tracks);
    ui_loop(root.get(), base, token);

    curl_global_cleanup();
    return 0;
}

