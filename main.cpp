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
// Data structures
// ─────────────────────────────────────────────────────────────────────────────

struct Track {
    std::string id, name, album, artist;
};

struct Node {
    std::string name;
    Track* track;                   // non-null only for leaves
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
// Helpers: sorting, tree‐flatten, curl callbacks
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
    for (auto& [k,v] : headers)
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
    struct curl_slist* hdrs = curl_slist_append(nullptr,
                                                "Content-Type: application/json");
    for (auto& [k,v] : headers)
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

void sort_tree(Node* node) {
    std::sort(node->children.begin(), node->children.end(),
              [](auto& a, auto& b){ return a->name < b->name; });
    for (auto& c : node->children) sort_tree(c.get());
}

void flatten(Node* node, std::vector<Node*>& out) {
    for (auto& c : node->children) {
        out.push_back(c.get());
        if (c->expanded) flatten(c.get(), out);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Config & Auth
// ─────────────────────────────────────────────────────────────────────────────

json load_config(const std::string& path) {
    std::ifstream in(path);
    if (in) { json j; in >> j; return j; }
    // first‐run prompt
    std::string url, user, pass;
    std::cout<<"Jellyfin server URL: "; std::getline(std::cin, url);
    std::cout<<"Username: ";          std::getline(std::cin, user);
    std::cout<<"Password: ";          std::getline(std::cin, pass);
    json j = {{"server_url",url},{"username",user},{"password",pass}};
    std::ofstream out(path);
    out<<j.dump(2)<<"\n";
    return j;
}

std::tuple<std::string,std::string,std::string>
authenticate(const json& cfg) {
    auto base = cfg.at("server_url").get<std::string>();
    if (!base.empty() && base.back()=='/') base.pop_back();
    std::vector<std::string> cand={base,base+"/jellyfin"};
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
            auto r = http_post_json(c+"/Users/AuthenticateByName",
                                    payload, hdrs);
            auto token = r.value("AccessToken","");
            auto user  = r.value("User", json::object());
            auto uid   = user.value("Id","");
            if (!token.empty() && !uid.empty()) return {token,uid,c};
        } catch(...) {}
    }
    std::cerr<<"Authentication failed.\n"; std::exit(1);
}

// ─────────────────────────────────────────────────────────────────────────────
// Fetch Tracks
// ─────────────────────────────────────────────────────────────────────────────

std::vector<Track> fetch_tracks(const std::string& base,
                                const std::string& token,
                                const std::string& user_id) {
    std::vector<Track> out;
    int start=0, limit=10000;
    auto hdrs = std::map<std::string,std::string>{{"X-Emby-Token",token}};
    while (true) {
        auto r = http_get_json(
          base+"/Users/"+user_id+"/Items?IncludeItemTypes=Audio"
               "&Recursive=true&SortBy=Album,SortName&SortOrder=Ascending"
               "&StartIndex="+std::to_string(start)+
               "&Limit="+std::to_string(limit),
          hdrs);
        auto items = r.value("Items", json::array());
        if (items.empty()) break;
        for (auto& it : items) {
            out.push_back({
                it.value("Id",""), it.value("Name","Unknown"),
                it.value("Album","Unknown"),
                !it.value("AlbumArtist","").empty()
                  ? it.value("AlbumArtist","")
                  : (!it.value("Artists",json::array()).empty()
                      ? it["Artists"][0].value("Name","Unknown")
                      : std::string("Unknown"))
            });
        }
        if ((int)items.size()<limit) break;
        start+=limit;
    }
    return out;
}

// ─────────────────────────────────────────────────────────────────────────────
// Build Tree (collapsed by default, sorted alphabetically)
// ─────────────────────────────────────────────────────────────────────────────

std::unique_ptr<Node> build_tree(const std::vector<Track>& tracks) {
    auto root = std::make_unique<Node>("Music Library", nullptr, nullptr);
    std::map<std::string,Node*> amap;
    for (auto& t : tracks) {
        Node* an;
        auto it = amap.find(t.artist);
        if (it==amap.end()) {
            an = new Node(t.artist,nullptr,root.get());
            root->children.emplace_back(an);
            amap[t.artist]=an;
        } else an=it->second;
        Node* alb=nullptr;
        for (auto& c:an->children) if(c->name==t.album){alb=c.get();break;}
        if(!alb){
            alb=new Node(t.album,nullptr,an);
            an->children.emplace_back(alb);
        }
        alb->children.emplace_back(
            std::make_unique<Node>(t.name,const_cast<Track*>(&t),alb)
        );
    }
    sort_tree(root.get());
    return root;
}

// ─────────────────────────────────────────────────────────────────────────────
// UI Loop: panels & tree navigation
// ─────────────────────────────────────────────────────────────────────────────

void ui_loop(Node* root,
             const std::string& base,
             const std::string& token) {
    initscr(); cbreak(); noecho(); keypad(stdscr, TRUE);
    int rows, cols; getmaxyx(stdscr, rows, cols);
    int ctrl_h = 1, info_w = cols/4;
    int main_h = rows - ctrl_h, main_w = cols - info_w;

    WINDOW* main_win     = newwin(main_h, main_w, 0, 0);
    WINDOW* info_win     = newwin(main_h, info_w, 0, main_w);
    WINDOW* controls_win = newwin(ctrl_h, cols, main_h, 0);

    std::vector<Node*> visible;
    flatten(root, visible);

    size_t cursor = 0, win_top = 0;
    libvlc_instance_t* vlc = libvlc_new(0,nullptr);
    libvlc_media_player_t* player = nullptr;
    Node* playing_node = nullptr;

    // initial draw
    flatten(root, visible);

    int ch;
    while ((ch = getch()) != 'q') {
        Node* cur = visible[cursor];
        switch (ch) {
            case KEY_UP:
                if (cursor>0) cursor--;
                break;
            case KEY_DOWN:
                if (cursor+1<visible.size()) cursor++;
                break;
            case KEY_RIGHT:
                if (!cur->children.empty()) cur->expanded = true;
                break;
            case KEY_LEFT:
                if (cur->expanded) cur->expanded = false;
                else if (cur->parent) {
                    // jump to parent
                    for (size_t i=0;i<visible.size();++i)
                        if (visible[i]==cur->parent) { cursor=i; break; }
                }
                break;
            case '\n':
                if (cur->track) {
                    if (player) libvlc_media_player_stop(player);
                    std::string url = base+"/Audio/"+cur->track->id
                      +"/universal?AudioCodec=mp3&Container=mp3&api_key="+token;
                    auto m = libvlc_media_new_location(vlc,url.c_str());
                    player = libvlc_media_player_new_from_media(m);
                    libvlc_media_release(m);
                    libvlc_media_player_play(player);
                    playing_node = cur;
                }
                break;
            default: break;
        }
        // rebuild visible and adjust window offset
        visible.clear();
        flatten(root, visible);
        int data_lines = main_h - 2; // account for border
        if (cursor < win_top) win_top = cursor;
        else if (cursor >= win_top + data_lines) win_top = cursor - data_lines + 1;

        // DRAW MAIN PANEL
        werase(main_win);
        box(main_win,0,0);
        int y = 1;
        for (size_t i=win_top; i<visible.size() && y<main_h-1; ++i,++y) {
            Node* n = visible[i];
            std::string indent(n->depth*2, ' ');
            if (i==cursor) wattron(main_win,A_REVERSE);
            mvwprintw(main_win,y,1,"%s%s",
                      indent.c_str(), n->name.c_str());
            if (i==cursor) wattroff(main_win,A_REVERSE);
        }
        wnoutrefresh(main_win);

        // DRAW INFO PANEL
        werase(info_win);
        box(info_win,0,0);
        int iy = 1;
        mvwprintw(info_win,iy++,1,"Selected:");
        if (cur->track) {
            mvwprintw(info_win,iy++,1,"Type: Track");
            mvwprintw(info_win,iy++,1,"Name: %s",cur->name.c_str());
            mvwprintw(info_win,iy++,1,"Album: %s",cur->parent->name.c_str());
            mvwprintw(info_win,iy++,1,"Artist: %s",cur->parent->parent->name.c_str());
        } else {
            mvwprintw(info_win,iy++,1,"Type: %s",
                      cur->depth==1?"Artist":"Album");
            mvwprintw(info_win,iy++,1,"Name: %s",cur->name.c_str());
            mvwprintw(info_win,iy++,1,"%s count: %d",
                      cur->depth==1?"Albums":"Tracks",
                      (int)cur->children.size());
        }
        if (playing_node) {
            mvwprintw(info_win,iy+1,1,"Now Playing:");
            mvwprintw(info_win,iy+2,1,"%s",playing_node->name.c_str());
        }
        wnoutrefresh(info_win);

        // DRAW CONTROLS PANEL
        werase(controls_win);
        wattron(controls_win,A_REVERSE);
        mvwprintw(controls_win,0,0,
          "↑/↓ scroll  → expand  ← collapse  Enter=play  q=quit");
        wattroff(controls_win,A_REVERSE);
        wnoutrefresh(controls_win);

        doupdate();
    }

    if (player) {
        libvlc_media_player_stop(player);
        libvlc_media_player_release(player);
    }
    libvlc_release(vlc);
    endwin();
}

// ─────────────────────────────────────────────────────────────────────────────
// main()
// ─────────────────────────────────────────────────────────────────────────────

int main() {
    curl_global_init(CURL_GLOBAL_DEFAULT);
    std::string cfg = std::string(getenv("HOME")) + "/.caitunes_config.json";
    auto jcfg = load_config(cfg);
    auto [token,user,base] = authenticate(jcfg);
    auto tracks = fetch_tracks(base, token, user);
    auto root = build_tree(tracks);
    ui_loop(root.get(), base, token);
    curl_global_cleanup();
    return 0;
}

