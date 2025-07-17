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
#include <random>
#include <locale.h>

#include <curl/curl.h>
#include <ncurses.h>
#include <vlc/vlc.h>
#include <nlohmann/json.hpp>

using json = nlohmann::json;

const std::string VERSION = "1.0";
// ─────────────────────────────────────────────────────────────────────────────
// Data structures
// ─────────────────────────────────────────────────────────────────────────────

struct Track {
    std::string id, name, album, artist;
};

struct Node {
    std::string name;
    Track* track;  // non-null only for leaf nodes
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
// Helpers: sort, flatten, collect, CURL callbacks
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
    std::string resp, body = payload.dump();
    struct curl_slist* hdrs = curl_slist_append(nullptr, "Content-Type: application/json");
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
    for (auto& c : node->children) {
        sort_tree(c.get());
    }
}

void flatten(Node* node, std::vector<Node*>& out) {
    for (auto& c : node->children) {
        out.push_back(c.get());
        if (c->expanded) flatten(c.get(), out);
    }
}

void collect_tracks(Node* node, std::vector<Node*>& out) {
    for (auto& c : node->children) {
        if (c->track) out.push_back(c.get());
        else collect_tracks(c.get(), out);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Config & Auth
// ─────────────────────────────────────────────────────────────────────────────

json load_config(const std::string& path) {
    std::ifstream in(path);
    if (in) { json j; in >> j; return j; }
    std::string url, user, pass;
    std::cout << "Jellyfin server URL: "; std::getline(std::cin, url);
    std::cout << "Username: ";          std::getline(std::cin, user);
    std::cout << "Password: ";          std::getline(std::cin, pass);
    json j = {{"server_url",url},{"username",user},{"password",pass}};
    std::ofstream out(path);
    out << j.dump(2) << "\n";
    return j;
}

std::tuple<std::string,std::string,std::string>
authenticate(const json& cfg) {
    auto base = cfg.at("server_url").get<std::string>();
    if (!base.empty() && base.back()=='/') base.pop_back();
    std::vector<std::string> cand = {base, base+"/jellyfin"};
    json payload = {
        {"Username", cfg.at("username").get<std::string>()},
        {"Pw",       cfg.at("password").get<std::string>()}
    };
    auto hdrs = std::map<std::string,std::string>{
        {"X-Emby-Authorization",
         R"(MediaBrowser Client="TUI", Device="cli", DeviceId="aitunes", Version=")" + VERSION + R"(")"}
    };
    for (auto& c : cand) {
        try {
            auto r = http_post_json(c+"/Users/AuthenticateByName", payload, hdrs);
            auto token = r.value("AccessToken", "");
            auto user  = r.value("User", json::object());
            auto uid   = user.value("Id", "");
            if (!token.empty() && !uid.empty()) return {token, uid, c};
        } catch(...) {}
    }
    std::cerr << "Authentication failed.\n";
    std::exit(1);
}

// ─────────────────────────────────────────────────────────────────────────────
// Fetch Tracks
// ─────────────────────────────────────────────────────────────────────────────

std::vector<Track> fetch_tracks(const std::string& base,
                                const std::string& token,
                                const std::string& user_id) {
    std::vector<Track> out;
    int start = 0, limit = 10000;
    auto hdrs = std::map<std::string,std::string>{{"X-Emby-Token", token}};
    while (true) {
        auto r = http_get_json(
          base + "/Users/" + user_id +
          "/Items?IncludeItemTypes=Audio&Recursive=true"
          "&SortBy=Album,SortName&SortOrder=Ascending"
          "&StartIndex=" + std::to_string(start) +
          "&Limit=" + std::to_string(limit),
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
                  : (!it.value("Artists",json::array()).empty()
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
// Build Tree (collapsed by default, sorted)
// ─────────────────────────────────────────────────────────────────────────────

std::unique_ptr<Node> build_tree(const std::vector<Track>& tracks) {
    auto root = std::make_unique<Node>("Music Library", nullptr, nullptr);
    std::map<std::string,Node*> amap;
    for (auto& t : tracks) {
        Node* an;
        auto it = amap.find(t.artist);
        if (it == amap.end()) {
            an = new Node(t.artist, nullptr, root.get());
            root->children.emplace_back(an);
            amap[t.artist] = an;
        } else an = it->second;
        Node* alb = nullptr;
        for (auto& c : an->children)
            if (c->name == t.album) { alb = c.get(); break; }
        if (!alb) {
            alb = new Node(t.album, nullptr, an);
            an->children.emplace_back(alb);
        }
        alb->children.emplace_back(
            std::make_unique<Node>(t.name, const_cast<Track*>(&t), alb)
        );
    }
    sort_tree(root.get());
    return root;
}

// ─────────────────────────────────────────────────────────────────────────────
// UI Loop with Queuing, Focus & Auto-Advance,
// Play/Pause, Volume (PgUp/Dn), Shuffle (⤨)
// ─────────────────────────────────────────────────────────────────────────────

enum Focus { TREE_FOCUSED, QUEUE_FOCUSED };

void ui_loop(Node* root,
             const std::string& base,
             const std::string& token) {
    setlocale(LC_ALL, "");
    initscr();

    // ── New Theme: text=CYAN on BLACK, controls text=BLACK on CYAN ────────────
    if (has_colors()) {
      start_color();
      use_default_colors();
      init_pair(1, COLOR_CYAN, COLOR_BLACK);  // main/info/queue
      init_pair(2, COLOR_BLACK, COLOR_CYAN);  // controls
    }
    // ──────────────────────────────────────────────────────────────────────────

    curs_set(0);
    cbreak();
    noecho();
    keypad(stdscr, TRUE);
    halfdelay(10);

    int rows, cols; getmaxyx(stdscr, rows, cols);
    int ctrl_h   = 1;
    int info_w   = cols / 4;
    int main_h   = rows - ctrl_h;
    int main_w   = cols - info_w;
    int info_h   = main_h / 2;
    int queue_h  = main_h - info_h;

    WINDOW* main_win     = newwin(main_h, main_w,     0,        0);
    WINDOW* info_win     = newwin(info_h, info_w,     0, main_w);
    WINDOW* queue_win    = newwin(queue_h, info_w, info_h, main_w);
    WINDOW* controls_win = newwin(ctrl_h, cols,  main_h,        0);

    std::vector<Node*> visible, queueList;
    size_t cursor      = 0, win_top     = 0;
    size_t queueCursor = 0, queue_top   = 0;
    Focus focus        = TREE_FOCUSED;

    int  volume = 50;
    bool paused = false;

    std::random_device rd;
    std::mt19937 rng(rd());

    libvlc_instance_t* vlc    = libvlc_new(0,nullptr);
    libvlc_media_player_t* player = nullptr;
    Node* playing_node = nullptr;

    auto draw_ui = [&]() {
        visible.clear();
        flatten(root, visible);

        // MAIN PANEL
        wbkgd(main_win, has_colors() ? COLOR_PAIR(1) : A_NORMAL);
        werase(main_win);
        wattron(main_win, has_colors() ? COLOR_PAIR(1) : A_NORMAL);
        box(main_win, 0, 0);
        wattroff(main_win, has_colors() ? COLOR_PAIR(1) : A_NORMAL);
        int y = 1, data_lines = main_h - 2;
        for (size_t idx = win_top; idx < visible.size() && y < main_h-1; ++idx, ++y) {
            Node* n = visible[idx];
            int x = 1;
            if (n->depth > 0) {
                for (int d = 1; d < n->depth; ++d) {
                    mvwaddch(main_win, y, x, ACS_VLINE);
                    x += 2;
                }
                bool last = (n->parent->children.back().get() == n);
                mvwaddch(main_win, y, x, last ? ACS_LLCORNER : ACS_LTEE);
                x++; mvwaddch(main_win, y, x, ACS_HLINE); x += 2;
            }
            if (focus==TREE_FOCUSED && idx==cursor) wattron(main_win, A_REVERSE);
            mvwprintw(main_win, y, x, "%s", n->name.c_str());
            if (focus==TREE_FOCUSED && idx==cursor) wattroff(main_win, A_REVERSE);
        }
        wnoutrefresh(main_win);

        // INFO PANEL
        wbkgd(info_win, has_colors() ? COLOR_PAIR(1) : A_NORMAL);
        werase(info_win);
        wattron(info_win, has_colors() ? COLOR_PAIR(1) : A_NORMAL);
        box(info_win, 0, 0);
        wattroff(info_win, has_colors() ? COLOR_PAIR(1) : A_NORMAL);
        int iy = 1;
        Node* cur = visible[cursor];
        mvwprintw(info_win,iy++,1,"Selected:");
        if (cur->track) {
            mvwprintw(info_win,iy++,1,"Track: %s", cur->name.c_str());
            mvwprintw(info_win,iy++,1,"Album: %s", cur->parent->name.c_str());
            mvwprintw(info_win,iy++,1,"Artist: %s", cur->parent->parent->name.c_str());
        } else {
            mvwprintw(info_win,iy++,1,"%s: %s",
                      cur->depth==1?"Artist":"Album", cur->name.c_str());
            mvwprintw(info_win,iy++,1,"%s count: %d",
                      cur->depth==1?"Albums":"Tracks",
                      (int)cur->children.size());
        }
        if (playing_node) {
            mvwprintw(info_win,iy+1,1,"Now Playing:");
            mvwprintw(info_win,iy+2,1,"%s", playing_node->name.c_str());
        }
        wnoutrefresh(info_win);

        // QUEUE PANEL
        wbkgd(queue_win, has_colors() ? COLOR_PAIR(1) : A_NORMAL);
        werase(queue_win);
        wattron(queue_win, has_colors() ? COLOR_PAIR(1) : A_NORMAL);
        box(queue_win, 0, 0);
        wattroff(queue_win, has_colors() ? COLOR_PAIR(1) : A_NORMAL);
        mvwprintw(queue_win,0,2," Queue ");
        int qy = 1, qlines = queue_h - 2;
        for (size_t i = queue_top; i < queueList.size() && qy < queue_h-1; ++i, ++qy) {
            if (focus==QUEUE_FOCUSED && i==queueCursor) wattron(queue_win, A_REVERSE);
            mvwprintw(queue_win, qy, 1, "%s", queueList[i]->name.c_str());
            if (focus==QUEUE_FOCUSED && i==queueCursor) wattroff(queue_win, A_REVERSE);
        }
        wnoutrefresh(queue_win);

        // CONTROLS PANEL
        wbkgd(controls_win, has_colors() ? COLOR_PAIR(2) : A_REVERSE);
        werase(controls_win);
        wattron(controls_win, has_colors() ? COLOR_PAIR(2) : A_REVERSE);
        const char* status_icon = paused ? "⏸" : " ▶";
        mvwprintw(controls_win, 0, 1,
                   "%s 🕪 %d%%  Nav: ↑ → ↓ ← ❘ Play: ⏎ ❘ ▶/⏸ : spcbar ❘ Vol: PgUp/Dn ❘ Add/Rm: F ❘⤨ : S ❘ Quit: Q",
                   status_icon, volume);
        wattroff(controls_win, has_colors() ? COLOR_PAIR(2) : A_REVERSE);
        wnoutrefresh(controls_win);

        doupdate();
    };

    draw_ui();

    int data_lines = main_h - 2;
    int ch;
    while (true) {
        ch = getch();
        bool handled = false;

        // Volume ↑/↓
        if (ch == KEY_PPAGE) {
            volume = std::min(100, volume + 5);
            if (player) libvlc_audio_set_volume(player, volume);
            handled = true;
        }
        else if (ch == KEY_NPAGE) {
            volume = std::max(0, volume - 5);
            if (player) libvlc_audio_set_volume(player, volume);
            handled = true;
        }
        // Ctrl + ↑/↓ fallback
        else if (ch == 27) {
            nodelay(stdscr, TRUE);
            int s1=getch(),s2=getch(),s3=getch(),s4=getch(),s5=getch();
            nodelay(stdscr, FALSE);
            if (s1=='[' && s2=='1' && s3==';' && s4=='5' && (s5=='A'||s5=='B')) {
                if (s5=='A') volume = std::min(100,volume+5);
                else          volume = std::max(0,volume-5);
                if (player) libvlc_audio_set_volume(player,volume);
                handled = true;
            }
        }
        // Shuffle
        else if (!handled && (ch=='S'||ch=='s')) {
            if (!queueList.empty()) {
                std::shuffle(queueList.begin(), queueList.end(), rng);
                queueCursor = queue_top = 0;
            }
            handled = true;
        }

        if (!handled) {
            if (ch==' ' && player) {
                paused = !paused;
                libvlc_media_player_set_pause(player, paused?1:0);
            }
            else if (ch=='\t') {
                focus = (focus==TREE_FOCUSED ? QUEUE_FOCUSED : TREE_FOCUSED);
            }
            else if (ch=='F'||ch=='f') {
                if (focus==TREE_FOCUSED) {
                    Node* cur = visible[cursor];
                    std::vector<Node*> to_add;
                    if (cur->track) to_add.push_back(cur);
                    else collect_tracks(cur,to_add);
                    for (auto n: to_add) queueList.push_back(n);
                } else if (!queueList.empty()) {
                    queueList.erase(queueList.begin()+queueCursor);
                    if (queueCursor>0) --queueCursor;
                }
            }
            else if (focus==TREE_FOCUSED) {
                Node* cur = visible[cursor];
                switch(ch) {
                  case KEY_UP:    if(cursor>0) --cursor; break;
                  case KEY_DOWN:  if(cursor+1<visible.size()) ++cursor; break;
                  case KEY_RIGHT: if(!cur->children.empty()) cur->expanded=true; break;
                  case KEY_LEFT:
                    if(cur->expanded) cur->expanded=false;
                    else if(cur->parent){
                      for(size_t i=0;i<visible.size();++i)
                        if(visible[i]==cur->parent){cursor=i;break;}
                    }
                    break;
                  case '\n':
                    if(cur->track){
                      if(player) libvlc_media_player_stop(player);
                      std::string url=base+"/Audio/"+cur->track->id
                        +"/universal?AudioCodec=mp3&Container=mp3&api_key="+token;
                      auto m=libvlc_media_new_location(vlc,url.c_str());
                      player=libvlc_media_player_new_from_media(m);
                      libvlc_media_release(m);
                      libvlc_media_player_play(player);
                      libvlc_audio_set_volume(player,volume);
                      paused=false;
                      playing_node=cur;
                    }
                    break;
                }
            } else { // QUEUE_FOCUSED
                switch(ch){
                  case KEY_UP:    if(queueCursor>0) --queueCursor; break;
                  case KEY_DOWN:  if(queueCursor+1<queueList.size()) ++queueCursor; break;
                  case '\n':
                    if(!queueList.empty()){
                      Node* cur=queueList[queueCursor];
                      if(player) libvlc_media_player_stop(player);
                      std::string url=base+"/Audio/"+cur->track->id
                        +"/universal?AudioCodec=mp3&Container=mp3&api_key="+token;
                      auto m=libvlc_media_new_location(vlc,url.c_str());
                      player=libvlc_media_player_new_from_media(m);
                      libvlc_media_release(m);
                      libvlc_media_player_play(player);
                      libvlc_audio_set_volume(player,volume);
                      paused=false;
                      playing_node=cur;
                    }
                    break;
                }
            }
        }

        // auto-advance
        if(player){
            auto state=libvlc_media_player_get_state(player);
            if(state==libvlc_Ended){
                if(!queueList.empty() && queueList.front()==playing_node){
                    queueList.erase(queueList.begin());
                    if(queueCursor>0) --queueCursor;
                }
                if(!queueList.empty()){
                    Node* next=queueList.front();
                    libvlc_media_player_stop(player);
                    std::string url=base+"/Audio/"+next->track->id
                      +"/universal?AudioCodec=mp3&Container=mp3&api_key="+token;
                    auto m=libvlc_media_new_location(vlc,url.c_str());
                    player=libvlc_media_player_new_from_media(m);
                    libvlc_media_release(m);
                    libvlc_media_player_play(player);
                    libvlc_audio_set_volume(player,volume);
                    paused=false;
                    playing_node=next;
                }
            }
        }

        // scroll
        if(focus==TREE_FOCUSED){
            if(cursor<win_top) win_top=cursor;
            else if(cursor>=win_top+data_lines) win_top=cursor-data_lines+1;
        } else {
            int ql=queue_h-2;
            if(queueCursor<queue_top) queue_top=queueCursor;
            else if(queueCursor>=queue_top+ql) queue_top=queueCursor-ql+1;
        }

        draw_ui();
        if(ch=='q') break;
    }

    if(player){
        libvlc_media_player_stop(player);
        libvlc_media_player_release(player);
    }
    libvlc_release(vlc);
    endwin();
}

int main(){
    curl_global_init(CURL_GLOBAL_DEFAULT);
    std::string cfg = "aitunes_config.json";
    json cfgj = load_config(cfg);
    std::cout << "AITUNES v" << VERSION << std::endl;
    auto [token,user,base] = authenticate(cfgj);
    std::cout << "🕪 Loading Tracks, please wait..." << std::endl;
    auto tracks = fetch_tracks(base,token,user);
    auto root = build_tree(tracks);
    ui_loop(root.get(),base,token);
    curl_global_cleanup();
    std::cout << "Thanks for vibing, goodbye." << std::endl;
    return 0;
}

